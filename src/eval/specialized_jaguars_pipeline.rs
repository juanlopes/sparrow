use crate::overlap::proxy::{bin_overlap_proxy, poly_overlap_proxy};
use crate::overlap::tracker::OverlapTracker;
use crate::util::assertions;
use crate::util::bit_reversal_iterator::BitReversalIterator;
use float_cmp::approx_eq;
use jagua_rs::collision_detection::cd_engine::CDEngine;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::collision_detection::hazard_helpers::{HazardDetector, HazardIgnorer};
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use slotmap::SecondaryMap;


/// Specialized collision collection function
/// Should behave the same as [`CDEngine::collect_poly_collisions_in_detector`], but with early termination.
/// Saving quite a bit of CPU time since over 90% of the time is spent in this function.

pub fn collect_poly_collisions_in_detector_specialized(
    cde: &CDEngine,
    shape: &SimplePolygon,
    det: &mut SpecializedDetectionMap,
) {
    //check a few poles to detect obvious collisions fast
    for pole in shape.surrogate().ff_poles() {
        cde.quadtree.collect_collisions(pole, det);
        if det.early_terminate(shape) {
            return;
        }
    }

    //go over all the edges of the shape, in a bit-reversed order to maximize detecting new hazards
    let custom_edge_iter = BitReversalIterator::new(shape.number_of_points())
        .map(|i| shape.get_edge(i));
    for edge in custom_edge_iter {
        cde.quadtree.collect_collisions(&edge, det);
        if det.early_terminate(shape) {
            return;
        }
    }

    //at this point, all hazards from edge-edge intersection are detected.
    //the only type of collision that can remain is containment.

    let checkpoint = det.idx_counter;

    //detect all potential hazards within the bounding box.
    cde.collect_potential_hazards_within(&shape.bbox(), det);

    if det.idx_counter > checkpoint {
        //there are additional hazards detected, we need to check if they are contained in the shape
        for haz in cde.all_hazards().filter(|h| h.active) {
            //go over all hazards in the CDE
            match haz.entity {
                HazardEntity::BinExterior => {
                    if let Some((_, idx)) = det.detected_bin {
                        if idx >= checkpoint {
                            //if the bin was detected as a potential containment, remove it.
                            //in this problem, an item can never be entirely outside the bin.
                            det.remove(&haz.entity)
                        }
                    }
                }
                HazardEntity::PlacedItem { pk, ..} => {
                    if let Some((_, idx)) = det.detected_pis.get(pk) {
                        if *idx >= checkpoint {
                            //the item was not detected during the quadtree query, but was detected as a potential containment.
                            if !cde.poly_or_hazard_are_contained(shape, haz) {
                                //if the item is not contained in the shape, remove it from the detector
                                det.remove(&haz.entity)
                            }
                        }
                    }
                }
                _ => unreachable!("unsupported hazard entity"),
            }
        }
    }
    debug_assert!(assertions::cc_matches_jaguars(shape, det));
}

/// Modified version of [`jagua_rs::collision_detection::hazard_helpers::DetectionMap`]
/// This struct computes the overlap incrementally, and caches the result.
/// This allows it to terminate early if the overlap exceeds a certain upperbound.
pub struct SpecializedDetectionMap<'a> {
    pub layout: &'a Layout,
    pub ot: &'a OverlapTracker,
    pub current_pk: PItemKey,
    pub detected_pis: SecondaryMap<PItemKey, (HazardEntity, usize)>,
    pub detected_bin: Option<(HazardEntity, usize)>,
    pub idx_counter: usize,
    pub loss_cache: (usize, f32),
    pub loss_bound: f32,
}

impl<'a> SpecializedDetectionMap<'a> {
    pub fn new(
        layout: &'a Layout,
        ot: &'a OverlapTracker,
        current_pk: PItemKey,
    ) -> Self {
        Self {
            layout,
            ot,
            current_pk,
            detected_pis: SecondaryMap::new(),
            detected_bin: None,
            idx_counter: 0,
            loss_cache: (0, 0.0),
            loss_bound : f32::INFINITY,
        }
    }
    pub fn reload(&mut self, loss_bound: f32) {
        self.detected_pis.clear();
        self.detected_bin = None;
        self.idx_counter = 0;
        self.loss_cache = (0, 0.0);
        self.loss_bound = loss_bound;
    }

    pub fn iter_with_index(&self) -> impl Iterator<Item=&(HazardEntity, usize)> {
        self.detected_pis.values().chain(self.detected_bin.iter())
    }

    pub fn early_terminate(&mut self, shape: &SimplePolygon) -> bool {
        self.loss(shape) > self.loss_bound
    }

    pub fn loss(&mut self, shape: &SimplePolygon) -> f32 {
        let (cache_idx, cached_loss) = self.loss_cache;
        if cache_idx < self.idx_counter {
            // additional hazards were detected, update the cache
            let extra_loss: f32 = self.iter_with_index()
                .filter(|(_, idx)| *idx >= cache_idx)
                .map(|(h, _)| self.calc_weighted_overlap(h, shape))
                .sum();
            self.loss_cache = (self.idx_counter, cached_loss + extra_loss);
        }
        debug_assert!(approx_eq!(f32, self.loss_cache.1, self.iter().map(|h| self.calc_weighted_overlap(h, shape)).sum()));
        self.loss_cache.1
    }

    fn calc_weighted_overlap(&self, haz: &HazardEntity, shape: &SimplePolygon) -> f32 {
        match haz {
            HazardEntity::PlacedItem { pk: other_pk, .. } => {
                let other_shape = &self.layout.placed_items[*other_pk].shape;
                let overlap = poly_overlap_proxy(shape, other_shape);
                let weight = self.ot.get_pair_weight(self.current_pk, *other_pk);
                overlap * weight
            }
            HazardEntity::BinExterior => {
                let overlap = bin_overlap_proxy(shape, self.layout.bin.bbox());
                let weight = self.ot.get_bin_weight(self.current_pk);
                2.0 * overlap * weight
            }
            _ => unimplemented!("unsupported hazard entity"),
        }
    }
}

impl<'a> HazardDetector for SpecializedDetectionMap<'a> {
    fn contains(&self, haz: &HazardEntity) -> bool {
        match haz {
            HazardEntity::PlacedItem { pk, .. } => {
                *pk == self.current_pk || self.detected_pis.contains_key(*pk)
            }
            HazardEntity::BinExterior => self.detected_bin.is_some(),
            _ => unreachable!("unsupported hazard entity"),
        }
    }

    fn push(&mut self, haz: HazardEntity) {
        debug_assert!(!self.contains(&haz));
        match haz {
            HazardEntity::PlacedItem { pk, .. } => {
                self.detected_pis.insert(pk, (haz, self.idx_counter));
            }
            HazardEntity::BinExterior => {
                self.detected_bin = Some((HazardEntity::BinExterior, self.idx_counter))
            },
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
            HazardEntity::BinExterior => {
                let (_, idx) = self.detected_bin.take().unwrap();
                if idx < self.loss_cache.0 {
                    //wipe the cache if a hazard was removed that was in it
                    self.loss_cache = (0, 0.0);
                }
            }
            _ => unreachable!("unsupported hazard entity"),
        }
    }

    fn len(&self) -> usize {
        self.detected_pis.len() + self.detected_bin.is_some() as usize
    }

    fn iter(&self) -> impl Iterator<Item=&HazardEntity> {
        self.detected_pis
            .iter()
            .map(|(_, (h, _))| h)
            .chain(self.detected_bin.iter().map(|(h, _)| h))
    }
}
impl<'a> HazardIgnorer for SpecializedDetectionMap<'a> {
    fn is_irrelevant(&self, haz: &HazardEntity) -> bool {
        self.contains(haz)
    }
}
