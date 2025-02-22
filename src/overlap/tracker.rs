use crate::io;
use crate::io::layout_to_svg::{layout_to_svg, layout_to_svg_2};
use crate::io::svg_util::SvgDrawOptions;
use crate::opt::gls_orchestrator::{JUMP_COOLDOWN, OT_DECAY, OT_MAX_INCREASE, OT_MIN_INCREASE};
use crate::overlap::{overlap_proxy};
use float_cmp::{approx_eq, assert_approx_eq};
use itertools::{Itertools, MinMaxResult};
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::util::assertions;
use jagua_rs::util::fpa::FPA;
use log::{debug, info, trace, warn};
use ordered_float::{Float, OrderedFloat};
use rand::Rng;
use slotmap::SecondaryMap;
use std::cmp::Ordering;
use std::iter;
use std::ops::Range;
use std::path::Path;
use crate::overlap::pair_matrix::PairMatrix;

#[derive(Debug, Clone, Copy)]
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

impl OTEntry {
    pub fn weighted_overlap(&self) -> fsize {
        self.weight * self.overlap
    }
}

pub struct OTSnapshot {
    pub pk_idx_map: SecondaryMap<PItemKey, usize>,
    pub pair_overlap: PairMatrix,
    pub bin_overlap: Vec<OTEntry>,
}

#[derive(Debug, Clone)]
pub struct OverlapTracker {
    pub capacity: usize,
    pub pk_idx_map: SecondaryMap<PItemKey, usize>,
    pub pair_overlap: PairMatrix,
    pub bin_overlap: Vec<OTEntry>,
    pub last_jump_iter: Vec<usize>,
    pub current_iter: usize,
}

impl OverlapTracker {
    pub fn from_layout(l: &Layout) -> Self {
        let capacity = l.placed_items.len();
        Self {
            capacity,
            pk_idx_map: SecondaryMap::with_capacity(capacity),
            pair_overlap: PairMatrix::new(capacity),
            bin_overlap: vec![OTEntry::default(); capacity],
            last_jump_iter: vec![0; capacity],
            current_iter: JUMP_COOLDOWN,
        }
            .init(l)
    }

    fn init(mut self, l: &Layout) -> Self {
        l.placed_items
            .keys()
            .enumerate()
            .for_each(|(i, pk)| {
                self.pk_idx_map.insert(pk, i);
            });


        l.placed_items
            .keys()
            .for_each(|pk| {
                self.recompute_overlap_for_item(pk, l)
            });

        debug_assert!(tracker_matches_layout(&self, l));

        self
    }

    fn recompute_overlap_for_item(&mut self, pk: PItemKey, l: &Layout) {
        let idx = self.pk_idx_map[pk];

        //reset the current overlap values
        self.pair_overlap.clear_overlaps_for(idx);
        self.bin_overlap[idx].overlap = 0.0;

        let pi = &l.placed_items[pk];
        let shape = pi.shape.as_ref();

        // Detect which hazards are overlapping with the item
        let overlapping = l.cde().collect_poly_collisions(shape, &[pi.into()]);

        // For each overlapping hazard, calculate the amount of overlap using the proxy functions
        // and store it in the overlap tracker
        for haz in overlapping {
            match haz {
                HazardEntity::PlacedItem { .. } => {
                    let other_pk = l.hazard_to_p_item_key(&haz).unwrap();
                    let other_shape = &l.placed_items[other_pk].shape;
                    let overlap = overlap_proxy::poly_overlap_proxy(shape, other_shape);

                    let other_idx = self.pk_idx_map[other_pk];

                    self.pair_overlap[(idx, other_idx)].overlap = overlap;
                }
                HazardEntity::BinExterior => {
                    warn!("bin exterior overlap");
                    let overlap = overlap_proxy::bin_overlap_proxy(shape, l.bin.bbox());
                    self.bin_overlap[idx].overlap = overlap;
                }
                _ => unimplemented!("unsupported hazard entity"),
            }
        }
    }

    pub fn restore_but_keep_weights(&mut self, ots: &OTSnapshot, layout: &Layout) {
        //copy the overlaps and keys, but keep the weights
        self.pk_idx_map = ots.pk_idx_map.clone();
        self.pair_overlap.data.iter_mut().zip(ots.pair_overlap.data.iter())
            .for_each(|(a, b)| a.overlap = b.overlap);
        self.bin_overlap.iter_mut().zip(ots.bin_overlap.iter())
            .for_each(|(a, b)| a.overlap = b.overlap);
        self.current_iter += JUMP_COOLDOWN; //fast-forward the weight iteration to the current iteration
        debug_assert!(tracker_matches_layout(self, layout));
    }

    pub fn create_snapshot(&self) -> OTSnapshot {
        OTSnapshot {
            pk_idx_map: self.pk_idx_map.clone(),
            pair_overlap: self.pair_overlap.clone(),
            bin_overlap: self.bin_overlap.clone(),
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
        let max_o = self.pair_overlap.data.iter()
            .map(|e| e.overlap)
            .fold(0.0, |a, b| a.max(b));

        for e in self.pair_overlap.data.iter_mut() {
            let multiplier = match e.overlap == 0.0 {
                true => OT_DECAY, // no overlap
                false => OT_MIN_INCREASE + (OT_MAX_INCREASE - OT_MIN_INCREASE) * (e.overlap / max_o),
            };
            let new_w = (e.weight * multiplier).max(1.0);
            e.weight = new_w;
        }

        for e in self.bin_overlap.iter_mut() {
            let multiplier = match e.overlap == 0.0 {
                true => OT_DECAY, // no overlap
                false => OT_MAX_INCREASE,
            };

            let new_w = (e.weight * multiplier).max(1.0);
            e.weight = new_w;
        }

        self.current_iter += 1;
    }

    pub fn get_pair_weight(&self, pk1: PItemKey, pk2: PItemKey) -> fsize {
        assert_ne!(pk1, pk2);
        let idx1 = self.pk_idx_map[pk1];
        let idx2 = self.pk_idx_map[pk2];

        self.pair_overlap[(idx1, idx2)].weight
    }

    pub fn get_bin_weight(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];
        self.bin_overlap[idx].weight
    }

    pub fn get_pair_overlap(&self, pk1: PItemKey, pk2: PItemKey) -> fsize {
        let idx1 = self.pk_idx_map[pk1];
        let idx2 = self.pk_idx_map[pk2];

        self.pair_overlap[(idx1, idx2)].overlap
    }

    pub fn get_bin_overlap(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];
        self.bin_overlap[idx].overlap
    }

    pub fn get_overlap(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];

        self.bin_overlap[idx].overlap +
            self.pair_overlap.row_iter(idx).map(|e| e.overlap).sum::<fsize>()
    }

    pub fn get_weighted_overlap(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];

        let w_bin_overlap = self.bin_overlap[idx].weighted_overlap();
        let w_pair_overlap = self.pair_overlap.row_iter(idx)
            .map(|e| e.weighted_overlap())
            .sum::<fsize>();

        w_bin_overlap + w_pair_overlap
    }

    pub fn get_total_overlap(&self) -> fsize {
        let bin_o = self.bin_overlap.iter()
            .map(|e| e.overlap).sum::<fsize>();

        let pair_o = self.pair_overlap.data.iter()
            .map(|e| e.overlap).sum::<fsize>();

        bin_o + pair_o
    }

    pub fn get_total_weighted_overlap(&self) -> fsize {
        let bin_w_o = self.bin_overlap.iter()
            .map(|e| e.weighted_overlap()).sum::<fsize>();

        let pair_w_o = self.pair_overlap.data.iter()
            .map(|e| e.weighted_overlap()).sum::<fsize>();

        bin_w_o + pair_w_o
    }

    pub fn register_jump(&mut self, pk: PItemKey) {
        let idx = self.pk_idx_map[pk];
        self.last_jump_iter[idx] = self.current_iter;
        trace!("[OT] jump for {:?} registered at iter: {}", pk, self.current_iter);
    }

    pub fn is_on_jump_cooldown(&self, pk: PItemKey) -> bool {
        let idx = self.pk_idx_map[pk];
        self.current_iter - self.last_jump_iter[idx] < JUMP_COOLDOWN
    }
}

pub fn tracker_matches_layout(ot: &OverlapTracker, l: &Layout) -> bool {
    assert!(l.placed_items.keys().all(|k| ot.pk_idx_map.contains_key(k)));
    assert!(assertions::layout_qt_matches_fresh_qt(l));

    for (pk1, pi1) in l.placed_items.iter() {
        let mut collisions = l.cde().collect_poly_collisions(&pi1.shape, &[pi1.into()]);
        assert_eq!(ot.get_pair_overlap(pk1, pk1), 0.0);
        for (pk2, pi2) in l.placed_items.iter().filter(|(k, _)| *k != pk1) {
            let stored_overlap = ot.get_pair_overlap(pk1, pk2);
            match collisions.contains(&(pi2.into())) {
                true => {
                    let calc_overlap =
                        overlap_proxy::poly_overlap_proxy(&pi1.shape, &pi2.shape);
                    let calc_overlap2 =
                        overlap_proxy::poly_overlap_proxy(&pi2.shape, &pi1.shape);
                    if !approx_eq!(
                        fsize,
                        calc_overlap,
                        stored_overlap,
                        epsilon = 0.001 * stored_overlap
                    ) {
                        let mut opposite_collisions = l.cde()
                            .collect_poly_collisions(&pi2.shape, &[pi2.into()]);
                        if opposite_collisions.contains(&(pi1.into())) {
                            dbg!(&pi1.shape.points, &pi2.shape.points);
                            dbg!(
                                stored_overlap,
                                calc_overlap,
                                calc_overlap2,
                                opposite_collisions,
                                HazardEntity::from(pi1),
                                HazardEntity::from(pi2)
                            );
                            panic!("overlap tracker error");
                        } else {
                            //detecting collisions is not symmetrical (in edge cases)
                            warn!("inconsistent overlap");
                            warn!("collisions: pi_1 {:?} -> {:?}", HazardEntity::from(pi1), collisions);
                            warn!("opposite collisions: pi_2 {:?} -> {:?}", HazardEntity::from(pi2), opposite_collisions);

                            warn!("pi_1: {:?}", pi1.shape.points.iter().map(|p| format!("({},{})", p.0, p.1)).collect_vec());
                            warn!("pi_2: {:?}", pi2.shape.points.iter().map(|p| format!("({},{})", p.0, p.1)).collect_vec());

                            {
                                let mut svg_draw_options = SvgDrawOptions::default();
                                svg_draw_options.quadtree = true;
                                let svg = layout_to_svg_2(l, svg_draw_options);
                                io::write_svg(&svg, &*Path::new("debug.svg"));
                                panic!("overlap tracker error");
                            }
                        }
                    }
                }
                false => {
                    if stored_overlap != 0.0 {
                        let calc_overlap =
                            overlap_proxy::poly_overlap_proxy(&pi1.shape, &pi2.shape);
                        let mut opposite_collisions = l.cde()
                            .collect_poly_collisions(&pi2.shape, &[pi2.into()]);
                        if !opposite_collisions.contains(&(pi1.into())) {
                            dbg!(&pi1.shape.points, &pi2.shape.points);
                            dbg!(
                                stored_overlap,
                                calc_overlap,
                                opposite_collisions,
                                HazardEntity::from(pi1),
                                HazardEntity::from(pi2)
                            );
                            panic!("overlap tracker error");
                        } else {
                            //detecting collisions is not symmetrical (in edge cases)
                            warn!("inconsistent overlap");
                            warn!("collisions: {:?} -> {:?}", HazardEntity::from(pi1), collisions);
                            warn!("opposite collisions: {:?} -> {:?}", HazardEntity::from(pi2), opposite_collisions);
                        }
                    }
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
