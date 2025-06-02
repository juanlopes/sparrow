use crate::quantify::quantify_collision_poly_container;
#[cfg(not(feature = "simd"))]
use crate::quantify::quantify_collision_poly_poly;
#[cfg(feature = "simd")]
use crate::quantify::simd::circles_soa::CirclesSoA;
#[cfg(feature = "simd")]
use crate::quantify::simd::quantify_collision_poly_poly_simd;
use crate::quantify::tracker::CollisionTracker;
use crate::util::assertions;
use crate::util::bit_reversal_iterator::BitReversalIterator;
use float_cmp::approx_eq;
use jagua_rs::collision_detection::CDEngine;
use jagua_rs::collision_detection::hazards::HazardEntity;
use jagua_rs::collision_detection::hazards::detector::HazardDetector;
use jagua_rs::collision_detection::quadtree::QTHazPresence;
use jagua_rs::entities::Layout;
use jagua_rs::entities::PItemKey;
use jagua_rs::geometry::DTransformation;
use jagua_rs::geometry::geo_traits::{TransformableFrom};
use jagua_rs::geometry::primitives::SPolygon;
use slotmap::SecondaryMap;

/// Functionally identical to [`CDEngine::collect_poly_collisions`], but with early return.
/// Collision collection will stop as soon as the loss exceeds the `loss_bound` of the detector.
/// Saving quite a bit of CPU time since over 90% of the time is spent in this function.
pub fn collect_poly_collisions_in_detector_custom(
    cde: &CDEngine,
    dt: &DTransformation,
    shape_buffer: &mut SPolygon,
    reference_shape: &SPolygon,
    det: &mut SpecializedHazardDetector,
) {
    let t = dt.compose();
    // transform the shape buffer to the new position
    let shape = shape_buffer.transform_from(reference_shape, &t);

    #[cfg(feature = "simd")]
    det.poles_soa.load(&shape.surrogate().poles);
    
    // Start off by checking a few poles to detect obvious collisions quickly
    for pole in shape.surrogate().ff_poles() {
        cde.quadtree.collect_collisions(pole, det);
        if det.early_terminate(shape) { return; }
    }

    // Find the virtual root of the quadtree for the shape's bounding box. So we do not have to start from the root every time.
    let v_qt_root = cde.get_virtual_root(shape.bbox);

    // Collect collisions for all edges.
    // Iterate over them in a bit-reversed order to maximize detecting new hazards early.
    let custom_edge_iter = BitReversalIterator::new(shape.n_vertices())
        .map(|i| shape.edge(i));
    for edge in custom_edge_iter {
        v_qt_root.collect_collisions(&edge, det);
        if det.early_terminate(shape) { return; }
    }

    // At this point, all collisions due to edge-edge intersection are detected.
    // The only type of collisions that possibly remain is containment.
    v_qt_root.hazards.active_hazards().iter().for_each(|qt_haz| {
        match &qt_haz.presence {
            QTHazPresence::None => (),
            // Hazards which are entirely present in the virtual root are guaranteed to be caught by any edge.
            QTHazPresence::Entire => (),
            QTHazPresence::Partial(qt_par_haz) => {
                if !det.contains(&qt_haz.entity) {
                    // Partially present hazards which are currently not detected have to be checked for containment.
                    if cde.detect_containment_collision(shape, &qt_par_haz.shape, qt_haz.entity)
                    {
                        det.push(qt_haz.entity);
                        if det.early_terminate(shape) { return; }
                    }
                }
            }
        }
    });
    
    // At this point, all collisions should be present in the detector.
    debug_assert!(assertions::custom_pipeline_matches_jaguars(shape, det), "Custom pipeline deviates from native jagua-rs pipeline");
}

/// Modified version of [`jagua_rs::collision_detection::hazards::detector::BasicHazardDetector`]
/// This struct computes the loss incrementally on the fly and caches the result.
/// Allows for early termination if the loss exceeds a certain upperbound.
pub struct SpecializedHazardDetector<'a> {
    pub layout: &'a Layout,
    pub ct: &'a CollisionTracker,
    pub current_pk: PItemKey,
    pub detected_pis: SecondaryMap<PItemKey, (HazardEntity, usize)>,
    pub detected_container: Option<(HazardEntity, usize)>,
    pub idx_counter: usize,
    pub loss_cache: (usize, f32),
    pub loss_bound: f32,
    #[cfg(feature = "simd")]
    pub poles_soa: CirclesSoA,
}

impl<'a> SpecializedHazardDetector<'a> {
    pub fn new(
        layout: &'a Layout,
        ct: &'a CollisionTracker,
        current_pk: PItemKey,
    ) -> Self {
        Self {
            layout,
            ct,
            current_pk,
            detected_pis: SecondaryMap::with_capacity(layout.placed_items.len()),
            detected_container: None,
            idx_counter: 0,
            loss_cache: (0, 0.0),
            loss_bound: f32::INFINITY,
            #[cfg(feature = "simd")]
            poles_soa: CirclesSoA::new(),
        }
    }

    pub fn reload(&mut self, loss_bound: f32) {
        self.detected_pis.clear();
        self.detected_container = None;
        self.idx_counter = 0;
        self.loss_cache = (0, 0.0);
        self.loss_bound = loss_bound;
    }

    pub fn iter_with_index(&self) -> impl Iterator<Item=&(HazardEntity, usize)> {
        self.detected_pis.values().chain(self.detected_container.iter())
    }

    pub fn early_terminate(&mut self, shape: &SPolygon) -> bool {
        self.loss(shape) > self.loss_bound
    }

    pub fn loss(&mut self, shape: &SPolygon) -> f32 {
        let (cache_idx, cached_loss) = self.loss_cache;
        if cache_idx < self.idx_counter {
            // additional hazards were detected, update the cache
            let extra_loss: f32 = self.iter_with_index()
                .filter(|(_, idx)| *idx >= cache_idx)
                .map(|(h, _)| self.calc_weighted_loss(h, shape))
                .sum();
            self.loss_cache = (self.idx_counter, cached_loss + extra_loss);
        }
        debug_assert!(approx_eq!(f32, self.loss_cache.1, self.iter().map(|h| self.calc_weighted_loss(h, shape)).sum()));
        self.loss_cache.1
    }

    fn calc_weighted_loss(&self, haz: &HazardEntity, shape: &SPolygon) -> f32 {
        match haz {
            HazardEntity::PlacedItem { pk: other_pk, .. } => {
                let other_shape = &self.layout.placed_items[*other_pk].shape;

                #[cfg(not(feature = "simd"))]
                let loss = quantify_collision_poly_poly(other_shape, shape);
                #[cfg(feature = "simd")]
                let loss = quantify_collision_poly_poly_simd(other_shape, shape, &self.poles_soa);

                let weight = self.ct.get_pair_weight(self.current_pk, *other_pk);
                loss * weight
            }
            HazardEntity::Exterior => {
                let loss = quantify_collision_poly_container(shape, self.layout.container.outer_cd.bbox);
                let weight = self.ct.get_container_weight(self.current_pk);
                loss * weight
            }
            _ => unimplemented!("unsupported hazard entity"),
        }
    }
}

impl<'a> HazardDetector for SpecializedHazardDetector<'a> {
    fn contains(&self, haz: &HazardEntity) -> bool {
        match haz {
            HazardEntity::PlacedItem { pk, .. } => {
                *pk == self.current_pk || self.detected_pis.contains_key(*pk)
            }
            HazardEntity::Exterior => self.detected_container.is_some(),
            _ => unreachable!("unsupported hazard entity"),
        }
    }

    fn push(&mut self, haz: HazardEntity) {
        debug_assert!(!self.contains(&haz));
        match haz {
            HazardEntity::PlacedItem { pk, .. } => {
                self.detected_pis.insert(pk, (haz, self.idx_counter));
            }
            HazardEntity::Exterior => {
                self.detected_container = Some((HazardEntity::Exterior, self.idx_counter))
            }
            _ => unreachable!("unsupported hazard entity"),
        }
        self.idx_counter += 1;
    }

    fn remove(&mut self, haz: &HazardEntity) {
        match haz {
            HazardEntity::PlacedItem { pk, .. } => {
                let (_, idx) = self.detected_pis.remove(*pk).unwrap();
                if idx < self.loss_cache.0 {
                    //wipe the cache if a hazard was removed that was in it
                    self.loss_cache = (0, 0.0);
                }
            }
            HazardEntity::Exterior => {
                let (_, idx) = self.detected_container.take().unwrap();
                if idx < self.loss_cache.0 {
                    //wipe the cache if a hazard was removed that was in it
                    self.loss_cache = (0, 0.0);
                }
            }
            _ => unreachable!("unsupported hazard entity"),
        }
    }

    fn len(&self) -> usize {
        self.detected_pis.len() + self.detected_container.is_some() as usize
    }

    fn iter(&self) -> impl Iterator<Item=&HazardEntity> {
        self.detected_pis.iter().map(|(_, (h, _))| h)
            .chain(self.detected_container.iter().map(|(h, _)| h))
    }
}