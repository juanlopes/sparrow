use std::collections::HashSet;
use crate::overlap::proxy::{bin_overlap_proxy, poly_overlap_proxy};
use crate::overlap::tracker::OverlapTracker;
use float_cmp::approx_eq;
use jagua_rs::collision_detection::cd_engine::CDEngine;
use jagua_rs::collision_detection::hazard::{HazardEntity};
use jagua_rs::collision_detection::hazard_helpers::{HazardDetector, HazardIgnorer};
use jagua_rs::collision_detection::quadtree::qt_hazard::QTHazPresence;
use jagua_rs::collision_detection::quadtree::qt_node::QTNode;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::geometry::geo_traits::{CollidesWith, Shape};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use slotmap::SecondaryMap;

pub fn collect_poly_collisions_in_detector2(
    cde: &CDEngine,
    shape: &SimplePolygon,
    detector: &mut DetectionMap2,
) {
    for pole in shape.surrogate().ff_poles() {
        cde.quadtree.collect_collisions(pole, detector);
        if detector.early_terminate(shape) {
            return;
        }
    }

    for edge in GeneralizedBitReversalIterator::new(shape.number_of_points()).map(|i| shape.get_edge(i)) {
        //for edge in shape.edge_iter(){
        cde.quadtree.collect_collisions(&edge, detector);
        if detector.early_terminate(shape) {
            return;
        }
    }
    // for edge in shape.edge_iter() {
    //     cde.quadtree.collect_collisions(&edge, detector);
    //     if detector.early_terminate(shape) {
    //         return;
    //     }
    // }


    //At this point, all hazards that edge-edge intersect must be caught.

    let bbox = shape.bbox();
    let det_idx = detector.index_counter();
    //detect all hazards within the bounding box
    hazards_within_bbox(&cde.quadtree, &bbox, detector);
    //cde.quadtree.collect_collisions(&bbox, detector);

    if detector.index_counter() > det_idx {
        //new hazards were detected.
        //For these we need to do a containment test
        cde.all_hazards().filter(|h| h.active).for_each(|h| {
            match h.entity {
                HazardEntity::PlacedItem { pk, .. } => {
                    //for all placed items, check if they are present in the detector
                    if let Some((_, idx)) = detector.detected_pis.get(pk) {
                        if *idx >= det_idx {
                            //undetected before
                            match cde.poly_or_hazard_are_contained(shape, h) {
                                true => (), //remain in the detector
                                false => {
                                    detector.remove(&h.entity)
                                }
                            }
                        }
                    }
                }
                HazardEntity::BinExterior => {
                    if let Some((_, idx)) = detector.detected_bin {
                        if idx >= det_idx {
                            detector.remove(&h.entity)
                        }
                    }
                },
                _ => panic!()
            }
        });
    }

    debug_assert!({
        let current_haz = (detector.current_pk, &detector.layout.placed_items[detector.current_pk]).into();
        let old_hazards = cde.collect_poly_collisions(&shape, &[current_haz]);
        //make sure these detection maps are equivalent
        let old_set: HashSet<HazardEntity> = old_hazards.iter().cloned().collect();
        let new_set: HashSet<HazardEntity> = detector.iter().cloned().collect();

        if old_set != new_set {
            dbg!(&old_set, &new_set, bbox);
            hazards_within_bbox(&cde.quadtree, &shape.bbox(), detector);
            panic!();
        }
        true
    })
}

/// Datastructure to register which Hazards are detected during collision collection.
/// Hazards caused by placed items have instant lookups, the others are stored in a Vec.
/// It also stores an index for each hazard, which can be used to determine the order in which they were detected.
pub struct DetectionMap2<'a> {
    pub layout: &'a Layout,
    pub ot: &'a OverlapTracker,
    pub current_pk: PItemKey,
    pub detected_pis: SecondaryMap<PItemKey, (HazardEntity, usize)>,
    pub detected_bin: Option<(HazardEntity, usize)>,
    pub idx_counter: usize,
    pub weighted_overlap_cache: (usize, f32),
    pub wo_upper_bound: f32,
}

impl<'a> DetectionMap2<'a> {
    pub fn reload(&mut self, wo_upper_bound: f32) {
        self.detected_pis.clear();
        self.detected_bin = None;
        self.idx_counter = 0;
        self.weighted_overlap_cache = (0, 0.0);
        self.wo_upper_bound = wo_upper_bound;
    }

    pub fn iter_with_index(&self) -> impl Iterator<Item=&(HazardEntity, usize)> {
        self.detected_pis.values().chain(self.detected_bin.iter())
    }

    pub fn index_counter(&self) -> usize {
        self.idx_counter
    }
    fn early_terminate(&mut self, shape: &SimplePolygon) -> bool {
        self.weighted_overlap(shape) > self.wo_upper_bound
    }

    pub fn weighted_overlap(&mut self, shape: &SimplePolygon) -> f32 {
        let (c_idx, c_wo) = self.weighted_overlap_cache;
        if c_idx < self.idx_counter {
            let n_wo: f32 = self.iter_with_index()
                .filter(|(_, idx)| *idx >= c_idx)
                .map(|(h, _)| self.calc_weighted_overlap(h, shape))
                .sum();
            self.weighted_overlap_cache = (self.idx_counter, c_wo + n_wo);
        }
        debug_assert!(approx_eq!(f32, self.weighted_overlap_cache.1, self.iter().map(|h| self.calc_weighted_overlap(h, shape)).sum::<f32>()));
        self.weighted_overlap_cache.1
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

impl<'a> HazardDetector for DetectionMap2<'a> {
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
            HazardEntity::BinExterior => self.detected_bin = Some((HazardEntity::BinExterior, self.idx_counter)),
            _ => unreachable!("unsupported hazard entity"),
        }
        self.idx_counter += 1;
    }

    fn remove(&mut self, haz: &HazardEntity) {
        match haz {
            HazardEntity::PlacedItem { pk, .. } => {
                let (_, idx) = self.detected_pis.remove(*pk).unwrap();
                if idx < self.weighted_overlap_cache.0 {
                    //wipe the cache if we removed an element that was in it
                    self.weighted_overlap_cache = (0, 0.0);
                }
            }
            HazardEntity::BinExterior => {
                let (_, idx) = self.detected_bin.take().unwrap();
                if idx < self.weighted_overlap_cache.0 {
                    //wipe the cache if we removed an element that was in it
                    self.weighted_overlap_cache = (0, 0.0);
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
impl<'a> HazardIgnorer for DetectionMap2<'a> {
    fn is_irrelevant(&self, haz: &HazardEntity) -> bool {
        self.contains(haz)
    }
}

struct GeneralizedBitReversalIterator {
    n: usize,      // Range size (0 to n-1)
    k: u32,      // Number of bits, smallest k where 2^k >= n
    i: usize,      // Current index
    count: usize,  // Number of elements yielded
}

impl GeneralizedBitReversalIterator {
    fn new(n: usize) -> Self {
        if n == 0 {
            Self { n: 0, k: 0, i: 0, count: 0 }
        } else {
            // Smallest k such that 2^k >= n
            let k = 64 - n.leading_zeros();
            Self { n, k, i: 0, count: 0 }
        }
    }
}

impl Iterator for GeneralizedBitReversalIterator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.count < self.n {
            // Reverse bits and shift to get k least significant bits
            let rev = self.i.reverse_bits() >> (64 - self.k);
            self.i += 1;
            if rev < self.n {
                self.count += 1;
                return Some(rev);
            }
        }
        None
    }
}

/// Gathers all hazards that are within a given boundingbox.
/// May overestimate the hazards, but never underestimate.
pub fn hazards_within_bbox(qtn: &QTNode, bbox: &AARectangle, detector: &mut impl HazardDetector){
    match bbox.collides_with(&qtn.bbox) {
        false => return, //Entity does not collide with the node
        true => match qtn.children.as_ref() {
            Some(children) => {
                //Do not perform any CD on this level, check the children
                children.iter().for_each(|child| {
                    hazards_within_bbox(child, bbox, detector);
                    //child.collect_collisions(bbox, detector);
                })
            }
            None => {
                //No children, detect all Entire hazards and check the Partial ones
                for hz in qtn.hazards.active_hazards().iter() {
                    match &hz.presence {
                        QTHazPresence::None => (),
                        QTHazPresence::Entire => {
                            if !detector.contains(&hz.entity) {
                                detector.push(hz.entity)
                            }
                        }
                        QTHazPresence::Partial(p_haz) => {
                            if !detector.contains(&hz.entity){
                                detector.push(hz.entity);
                            }
                        }
                    }
                }
            }
        },
    }
}
