use std::iter;
use std::ops::Range;
use float_cmp::assert_approx_eq;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::util::fpa::FPA;
use rand::Rng;
use slotmap::SecondaryMap;
use crate::overlap::{overlap, overlap_proxy};

#[derive(Clone, Copy, Debug)]
pub struct OTEntry {
    pub weight: fsize,
    pub overlap: fsize,
}

impl Default for OTEntry {
    fn default() -> Self {
        Self {
            weight: 1.0,
            overlap: 0.0,
        }
    }
}

const INCREMENT: fsize = 1.2;

#[derive(Clone, Debug)]
pub struct OverlapTracker {
    pub pair_overlap: SecondaryMap<PItemKey, SecondaryMap<PItemKey, OTEntry>>,
    pub bin_overlap: SecondaryMap<PItemKey, OTEntry>,
}

impl OverlapTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            pair_overlap: SecondaryMap::with_capacity(capacity),
            bin_overlap: SecondaryMap::with_capacity(capacity),
        }
    }

    pub fn sync(&mut self, l: &Layout) {
        //remove all the keys that are not in the layout
        let mut removed_keys = vec![];

        for pk in self.bin_overlap.keys() {
            if !l.placed_items.contains_key(pk) {
                removed_keys.push(pk);
            }
        }

        for pk in removed_keys {
            self.pair_overlap.remove(pk);
            for (_, m) in self.pair_overlap.iter_mut() {
                m.remove(pk);
            }
            self.bin_overlap.remove(pk);
        }

        //add all the keys that are in the layout but not in the tracker
        let mut new_keys = vec![];

        for pk in l.placed_items.keys() {
            if !self.pair_overlap.contains_key(pk) {
                new_keys.push(pk);
                //make sure all the maps include the key
                let fresh_map = SecondaryMap::from_iter(
                    iter::once((pk, OTEntry::default())).chain(
                    self.pair_overlap.keys().map(|k| (k, OTEntry::default()))),
                );
                self.pair_overlap.insert(pk, fresh_map);
                for (_, m) in self.pair_overlap.iter_mut() {
                    m.insert(pk, OTEntry::default());
                }
                self.bin_overlap.insert(pk, OTEntry::default());

            }
        }

        for pk in new_keys {
            self.compute_and_set_overlap_for_key(l, pk);
        }

        debug_assert!(tracker_symmetrical(self));
        debug_assert!(tracker_matches_layout(self, l));
    }

    pub fn move_item(&mut self, l: &Layout, old_key: PItemKey, new_key: PItemKey) {
        //update keys in the pair maps
        {
            for (k, m) in self.pair_overlap.iter_mut() {
                let old_ot_entry = m.remove(old_key).unwrap();
                let new_ot_entry = OTEntry {
                    weight: old_ot_entry.weight,
                    overlap: 0.0
                };
                m.insert(new_key, new_ot_entry);
            }
            let mut old_map = self.pair_overlap.remove(old_key).unwrap();
            old_map.remove(old_key);
            old_map.insert(new_key, OTEntry::default());
            old_map.iter_mut().for_each(|(_, e)| e.overlap = 0.0);
            self.pair_overlap.insert(new_key, old_map);
        }

        //update keys in the bin map
        {
            let old_bin_ot_entry = self.bin_overlap.remove(old_key).unwrap();
            let new_bin_ot_entry = OTEntry {
                weight: old_bin_ot_entry.weight,
                overlap: 0.0
            };
            self.bin_overlap.insert(new_key, new_bin_ot_entry);
        }

        self.compute_and_set_overlap_for_key(l, new_key);

        debug_assert!(tracker_symmetrical(self));
        debug_assert!(tracker_matches_layout(self, l));
    }

    fn compute_and_set_overlap_for_key(&mut self, l: &Layout, pk: PItemKey){
        let pi = &l.placed_items[pk];
        let shape = pi.shape.as_ref();
        let mut ol_haz = vec![];
        l.cde().collect_poly_collisions(shape, &[pi.into()], &mut ol_haz);
        for haz in ol_haz {
            match haz {
                HazardEntity::PlacedItem { .. } => {
                    let other_pk = l.hazard_to_p_item_key(&haz).unwrap();
                    let other_shape = &l.placed_items[other_pk].shape;
                    let overlap = overlap_proxy::poly_overlap_proxy(shape, other_shape, l.bin.bbox());
                    self.pair_overlap[pk][other_pk].overlap = overlap;
                    self.pair_overlap[other_pk][pk].overlap = overlap;
                }
                HazardEntity::BinExterior => {
                    let overlap = overlap_proxy::bin_overlap_proxy(shape, l.bin.bbox());
                    self.bin_overlap[pk].overlap = overlap;
                }
                _ => {}
            }
        }
    }

    pub fn increment_weights(&mut self) {
        for (_, m) in self.pair_overlap.iter_mut() {
            for (_, e) in m.iter_mut() {
                if e.overlap > 0.0 {
                    e.weight *= INCREMENT;
                }
            }
        }
        for (_, e) in self.bin_overlap.iter_mut() {
            if e.overlap > 0.0 {
                e.weight *= INCREMENT;
            }
        }
        debug_assert!(tracker_symmetrical(self));
    }

    pub fn randomize_weights(&mut self, range: Range<fsize>, rng: &mut impl Rng) {
        for e in self.bin_overlap.values_mut() {
            e.weight = rng.gen_range(range.clone());
        }
        for pk1 in self.bin_overlap.keys() {
            for pk2 in self.bin_overlap.keys() {
                if pk1 <= pk2 {
                    let random_weight = rng.gen_range(range.clone());
                    self.pair_overlap[pk1][pk2].weight = random_weight;
                    self.pair_overlap[pk2][pk1].weight = random_weight;
                }
            }
        }
        assert!(tracker_symmetrical(self));
    }

    pub fn get_pair_weight(&self, pk1: PItemKey, pk2: PItemKey) -> fsize {
        self.pair_overlap[pk1][pk2].weight
    }

    pub fn get_bin_weight(&self, pk: PItemKey) -> fsize {
        self.bin_overlap[pk].weight
    }

    pub fn get_pair_overlap(&self, pk1: PItemKey, pk2: PItemKey) -> fsize {
        self.pair_overlap[pk1][pk2].overlap
    }

    pub fn get_bin_overlap(&self, pk: PItemKey) -> fsize {
        self.bin_overlap[pk].overlap
    }

    pub fn get_overlap(&self, pk: PItemKey) -> fsize {
        self.bin_overlap.get(pk).map_or(0.0, |e| e.overlap) +
        self.pair_overlap.get(pk).map_or(0.0, |m| m.values().map(|e| e.overlap).sum())
    }

    pub fn get_weighted_overlap(&self, pk: PItemKey) -> fsize {
        self.bin_overlap.get(pk)
            .map_or(0.0, |e| e.overlap * e.weight) +
        self.pair_overlap.get(pk)
            .map_or(0.0, |m|
                m.values()
                    .map(|e| e.overlap * e.weight)
                    .sum()
            )
    }

    pub fn get_total_overlap(&self) -> fsize {
        self.bin_overlap.keys()
            .map(|pk| self.get_overlap(pk))
            .sum()
    }

    pub fn get_total_weighted_overlap(&self) -> fsize {
        self.bin_overlap.keys()
            .map(|pk| self.get_weighted_overlap(pk))
            .sum()
    }
}

fn tracker_symmetrical(ot: &OverlapTracker) -> bool {
    for pk1 in ot.pair_overlap.keys() {
        for pk2 in ot.pair_overlap.keys() {
            let ot1 = ot.pair_overlap[pk1][pk2];
            let ot2 = ot.pair_overlap[pk2][pk1];

            assert_approx_eq!(fsize, ot1.weight, ot2.weight, epsilon = FPA::tolerance());
            assert_approx_eq!(fsize, ot1.overlap, ot2.overlap, epsilon = FPA::tolerance());
        }
    }
    true
}

fn tracker_matches_layout(ot: &OverlapTracker, l: &Layout) -> bool {
    assert!(ot
        .pair_overlap
        .keys()
        .all(|k| l.placed_items.contains_key(k)));
    assert!(ot
        .bin_overlap
        .keys()
        .all(|k| l.placed_items.contains_key(k)));

    for (pk1, pi1) in l.placed_items.iter() {
        let mut collisions = vec![];
        l.cde().collect_poly_collisions(&pi1.shape, &[pi1.into()], &mut collisions);
        assert_eq!(ot.get_pair_overlap(pk1, pk1), 0.0);
        for (pk2, pi2) in l.placed_items.iter().filter(|(k, _)| *k != pk1) {
            let stored_overlap = ot.get_pair_overlap(pk1, pk2);
            match collisions.contains(&(pi2.into())) {
                true => {
                    let calc_overlap = overlap_proxy::poly_overlap_proxy(&pi1.shape, &pi2.shape, l.bin.bbox());
                    assert_approx_eq!(fsize, calc_overlap, stored_overlap, epsilon = FPA::tolerance());
                }
                false => {
                    assert_eq!(stored_overlap, 0.0);
                }
            }
        }
        if collisions.contains(&HazardEntity::BinExterior) {
            let bin_overlap = ot.get_bin_overlap(pk1);
            let calc_overlap = overlap_proxy::bin_overlap_proxy(&pi1.shape, l.bin.bbox());
            assert_approx_eq!(fsize, calc_overlap, bin_overlap, epsilon = FPA::tolerance());
        } else {
            assert_eq!(ot.get_bin_overlap(pk1), 0.0);
        }
    }

    true
}