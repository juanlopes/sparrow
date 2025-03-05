use crate::config::{WEIGHT_MAX_INC_RATIO, WEIGHT_MIN_INC_RATIO, WEIGHT_OVERLAP_DECAY};
use crate::overlap::pair_matrix::OTPairMatrix;
use crate::overlap::proxy;
use crate::util::assertions::tracker_matches_layout;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::collision_detection::hazard_helpers::HazardDetector;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use ordered_float::Float;
use slotmap::SecondaryMap;
use tap::Tap;

#[derive(Debug, Clone)]
pub struct OverlapTracker {
    pub size: usize,
    pub pk_idx_map: SecondaryMap<PItemKey, usize>,
    pub pair_overlap: OTPairMatrix,
    pub bin_overlap: Vec<OTEntry>,
}

impl OverlapTracker {
    pub fn new(l: &Layout) -> Self {
        let size = l.placed_items.len();
        let pk_idx_map = l.placed_items.keys().enumerate()
            .map(|(i, pk)| (pk, i)).collect();

        Self {
            size,
            pk_idx_map,
            pair_overlap: OTPairMatrix::new(size),
            bin_overlap: vec![OTEntry::default(); size],
        }.tap_mut(|ot| {
            l.placed_items.keys().for_each(|pk| {
                ot.recompute_overlap_for_item(pk, l)
            });
            debug_assert!(tracker_matches_layout(&ot, l));
        })
    }

    fn recompute_overlap_for_item(&mut self, pk: PItemKey, l: &Layout) {
        let idx = self.pk_idx_map[pk];

        //reset the current overlap values
        for i in 0..self.size {
            self.pair_overlap[(idx, i)].overlap = 0.0;
        }
        self.bin_overlap[idx].overlap = 0.0;

        let pi = &l.placed_items[pk];
        let shape = pi.shape.as_ref();

        // Detect which hazards are overlapping with the item
        let overlapping = l.cde().collect_poly_collisions(shape, &[(pk, pi).into()]);

        // For each overlapping hazard, calculate the amount of overlap using the proxy functions
        // and store it in the overlap tracker
        for haz in overlapping.iter() {
            match haz {
                HazardEntity::PlacedItem { pk: other_pk, .. } => {
                    let other_shape = &l.placed_items[*other_pk].shape;
                    let overlap = proxy::poly_overlap_proxy(shape, other_shape);

                    let other_idx = self.pk_idx_map[*other_pk];

                    self.pair_overlap[(idx, other_idx)].overlap = overlap;
                }
                HazardEntity::BinExterior => {
                    let overlap = proxy::bin_overlap_proxy(shape, l.bin.bbox());
                    self.bin_overlap[idx].overlap = overlap;
                }
                _ => unimplemented!("unsupported hazard entity"),
            }
        }
    }

    pub fn restore_but_keep_weights(&mut self, ots: &OTSnapshot, layout: &Layout) {
        //copy the overlaps and keys, but keep the weights
        self.pk_idx_map = ots.pk_idx_map.clone();
        self.pair_overlap
            .data
            .iter_mut()
            .zip(ots.pair_overlap.data.iter())
            .for_each(|(a, b)| a.overlap = b.overlap);
        self.bin_overlap
            .iter_mut()
            .zip(ots.bin_overlap.iter())
            .for_each(|(a, b)| a.overlap = b.overlap);
        debug_assert!(tracker_matches_layout(self, layout));
    }

    pub fn create_snapshot(&self) -> OTSnapshot {
        OTSnapshot {
            pk_idx_map: self.pk_idx_map.clone(),
            pair_overlap: self.pair_overlap.clone(),
            bin_overlap: self.bin_overlap.clone(),
            total_overlap: self.get_total_overlap(),
        }
    }

    pub fn register_item_move(&mut self, l: &Layout, old_pk: PItemKey, new_pk: PItemKey) {
        //swap the keys in the pk_idx_map
        let idx = self.pk_idx_map.remove(old_pk).unwrap();
        self.pk_idx_map.insert(new_pk, idx);

        self.recompute_overlap_for_item(new_pk, l);

        debug_assert!(tracker_matches_layout(self, l));
    }

    pub fn increment_weights(&mut self) {
        let max_o = self
            .pair_overlap
            .data
            .iter()
            .map(|e| e.overlap)
            .fold(0.0, |a, b| a.max(b));

        for e in self.pair_overlap.data.iter_mut() {
            let multiplier = match e.overlap == 0.0 {
                true => WEIGHT_OVERLAP_DECAY, // no overlap
                false => {
                    WEIGHT_MIN_INC_RATIO + (WEIGHT_MAX_INC_RATIO - WEIGHT_MIN_INC_RATIO) * (e.overlap / max_o)
                }
            };
            let new_w = (e.weight * multiplier).max(1.0);
            e.weight = new_w;
        }

        for e in self.bin_overlap.iter_mut() {
            let multiplier = match e.overlap == 0.0 {
                true => WEIGHT_OVERLAP_DECAY, // no overlap
                false => WEIGHT_MAX_INC_RATIO,
            };

            let new_w = (e.weight * multiplier).max(1.0);
            e.weight = new_w;
        }
    }

    pub fn get_pair_weight(&self, pk1: PItemKey, pk2: PItemKey) -> f32 {
        let (idx1, idx2) = (self.pk_idx_map[pk1], self.pk_idx_map[pk2]);
        self.pair_overlap[(idx1, idx2)].weight
    }

    pub fn get_bin_weight(&self, pk: PItemKey) -> f32 {
        let idx = self.pk_idx_map[pk];
        self.bin_overlap[idx].weight
    }

    pub fn get_pair_overlap(&self, pk1: PItemKey, pk2: PItemKey) -> f32 {
        let (idx1, idx2) = (self.pk_idx_map[pk1], self.pk_idx_map[pk2]);
        self.pair_overlap[(idx1, idx2)].overlap
    }

    pub fn get_bin_overlap(&self, pk: PItemKey) -> f32 {
        let idx = self.pk_idx_map[pk];
        self.bin_overlap[idx].overlap
    }

    pub fn get_overlap(&self, pk: PItemKey) -> f32 {
        let idx = self.pk_idx_map[pk];

        self.bin_overlap[idx].overlap
            + (0..self.size)
            .map(|i| self.pair_overlap[(idx, i)].overlap)
            .sum::<f32>()
    }

    pub fn get_weighted_overlap(&self, pk: PItemKey) -> f32 {
        let idx = self.pk_idx_map[pk];

        let w_bin_overlap = self.bin_overlap[idx].weighted_overlap();
        let w_pair_overlap = (0..self.size)
            .map(|i| self.pair_overlap[(idx, i)].weighted_overlap())
            .sum::<f32>();

        w_bin_overlap + w_pair_overlap
    }

    pub fn get_total_overlap(&self) -> f32 {
        let bin_o = self.bin_overlap.iter().map(|e| e.overlap).sum::<f32>();

        let pair_o = self
            .pair_overlap
            .data
            .iter()
            .map(|e| e.overlap)
            .sum::<f32>();

        bin_o + pair_o
    }

    pub fn get_total_weighted_overlap(&self) -> f32 {
        let bin_w_o = self.bin_overlap.iter()
            .map(|e| e.weighted_overlap())
            .sum::<f32>();

        let pair_w_o = self.pair_overlap.data.iter()
            .map(|e| e.weighted_overlap())
            .sum::<f32>();

        bin_w_o + pair_w_o
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OTEntry {
    pub weight: f32,
    pub overlap: f32,
}

impl Default for OTEntry {
    fn default() -> Self {
        Self { weight: 1.0, overlap: 0.0 }
    }
}

impl OTEntry {
    pub fn weighted_overlap(&self) -> f32 {
        self.weight * self.overlap
    }
}

pub struct OTSnapshot {
    pub pk_idx_map: SecondaryMap<PItemKey, usize>,
    pub pair_overlap: OTPairMatrix,
    pub bin_overlap: Vec<OTEntry>,
    pub total_overlap: f32,
}