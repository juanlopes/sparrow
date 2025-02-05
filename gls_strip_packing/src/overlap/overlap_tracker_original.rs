use std::cmp::Ordering;
use crate::overlap::matrix::{assert_matrix_symmetrical, Matrix};
use crate::overlap::{overlap, overlap_proxy};
use float_cmp::{approx_eq, assert_approx_eq};
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::util::fpa::FPA;
use log::{debug, info, warn};
use ordered_float::{Float, OrderedFloat};
use rand::Rng;
use slotmap::SecondaryMap;
use std::iter;
use std::ops::Range;
use itertools::{Itertools, MinMaxResult};
use jagua_rs::geometry::geo_traits::Shape;
use num_traits::real::Real;

pub struct OTSnapshot {
    pub pk_idx_map: SecondaryMap<PItemKey, usize>,
    pub pair_overlap: Matrix<fsize>,
    pub bin_overlap: Vec<fsize>,
}

#[derive(Debug, Clone)]
pub struct OverlapTracker {
    pub capacity: usize,
    pub pk_idx_map: SecondaryMap<PItemKey, usize>,
    pub pair_overlap: Matrix<fsize>,
    pub pair_weights: Matrix<fsize>,
    pub bin_overlap: Vec<fsize>,
    pub bin_weights: Vec<fsize>,
    pub item_mass: Vec<fsize>,
    pub last_jump: Vec<usize>,
    pub weight_rescale_target: fsize,
    pub weight_iter: usize,
    pub jump_cooldown: usize,
    pub weight_multiplier: fsize,
}

impl OverlapTracker {
    pub fn new(l: &Layout, weight_rescale_target: fsize, weight_multiplier: fsize, jump_cooldown: usize) -> Self {
        let capacity = l.placed_items.len();
        Self {
            capacity,
            pk_idx_map: SecondaryMap::with_capacity(capacity),
            pair_overlap: Matrix::new(capacity, 0.0),
            pair_weights: Matrix::new(capacity, 1.0),
            bin_overlap: vec![0.0; capacity],
            bin_weights: vec![1.0; capacity],
            item_mass: vec![fsize::NAN; capacity],
            last_jump: vec![0; capacity],
            weight_iter: jump_cooldown,
            jump_cooldown,
            weight_rescale_target,
            weight_multiplier,
        }
            .init(l)
    }

    fn init(mut self, l: &Layout) -> Self {
        for (i, (pk, pi)) in l.placed_items.iter().enumerate() {
            self.pk_idx_map.insert(pk, i);
            self.item_mass[i] = pi.shape.surrogate().convex_hull_area;
        }

        for pk in l.placed_items.keys() {
            self.recompute_overlap_for_item(pk, l);
        }

        debug_assert!(tracker_matches_layout(&self, l));

        self
    }

    fn recompute_overlap_for_item(&mut self, pk: PItemKey, l: &Layout) {
        let idx = self.pk_idx_map[pk];

        //reset the current overlap values
        self.pair_overlap.reset_row_and_col(idx);
        self.bin_overlap[idx] = 0.0;

        let pi = &l.placed_items[pk];
        let shape = pi.shape.as_ref();

        // Detect which hazards are overlapping with the item
        let overlapping = {
            let mut buffer = vec![];
            l.cde().collect_poly_collisions(shape, &[pi.into()], &mut buffer);
            buffer
        };

        // For each overlapping hazard, calculate the amount of overlap using the proxy functions
        // and store it in the overlap tracker
        for haz in overlapping {
            match haz {
                HazardEntity::PlacedItem { .. } => {
                    let other_pk = l.hazard_to_p_item_key(&haz).unwrap();
                    let other_shape = &l.placed_items[other_pk].shape;
                    let overlap = overlap_proxy::poly_overlap_proxy(shape, other_shape, l.bin.bbox());

                    let other_idx = self.pk_idx_map[other_pk];

                    self.pair_overlap[(idx, other_idx)] = overlap;
                    self.pair_overlap[(other_idx, idx)] = overlap;
                }
                HazardEntity::BinExterior => {
                    warn!("bin exterior overlap");
                    let overlap = overlap_proxy::bin_overlap_proxy(shape, l.bin.bbox());
                    self.bin_overlap[idx] = overlap;
                }
                _ => unimplemented!("unsupported hazard entity"),
            }
        }
    }

    pub fn restore(&mut self, ots: &OTSnapshot, layout: &Layout) {
        //copy the overlaps and keys, but keep the weights
        self.pk_idx_map = ots.pk_idx_map.clone();
        self.pair_overlap = ots.pair_overlap.clone();
        self.bin_overlap = ots.bin_overlap.clone();
        self.weight_iter += self.jump_cooldown; //fast-forward the weight iteration to the current iteration
        assert!(tracker_matches_layout(self, layout));
    }

    pub fn create_snapshot(&self) -> OTSnapshot {
        OTSnapshot {
            pk_idx_map: self.pk_idx_map.clone(),
            pair_overlap: self.pair_overlap.clone(),
            bin_overlap: self.bin_overlap.clone(),
        }
    }

    pub fn move_item(&mut self, l: &Layout, old_pk: PItemKey, new_pk: PItemKey) {
        //update the pk_idx_map
        let idx = self.pk_idx_map.remove(old_pk).unwrap();
        self.pk_idx_map.insert(new_pk, idx);

        self.recompute_overlap_for_item(new_pk, l);

        debug_assert!(tracker_matches_layout(self, l));
    }

    pub fn increment_weights(&mut self) {
        for idx1 in 0..self.capacity {
            for idx2 in 0..self.capacity {
                let o = self.pair_overlap[(idx1, idx2)];
                if o > 0.0 {
                    //compute increment mapping o between [min_overlap, max_overlap] to [MIN_INCREMENT, MAX_INCREMENT]
                    self.pair_weights[(idx1, idx2)] *= self.weight_multiplier;
                }
                else {
                    //self.pair_weights[(idx1, idx2)] *= 1.1;
                }
            }
            if self.bin_overlap[idx1] > 0.0 {
                self.bin_weights[idx1] *= self.weight_multiplier.powi(2);
            }
        }

        self.weight_iter += 1;

        debug_assert!(assert_matrix_symmetrical(&self.pair_overlap));
        debug_assert!(assert_matrix_symmetrical(&self.pair_weights));
    }

    pub fn rescale_weights(&mut self) {
        let target = self.weight_rescale_target;

        let max_weight = self.pair_weights.data.iter()
            .chain(self.bin_weights.iter())
            .max_by_key(|&w| OrderedFloat(*w))
            .copied()
            .unwrap();

        let rescale_factor = target / max_weight;

        for w in self.pair_weights.data.iter_mut() {
            *w = fsize::max(*w * rescale_factor, 1.0);
        }

        for w in self.bin_weights.iter_mut() {
            *w = fsize::max(*w * rescale_factor, 1.0);
        }

        let new_max = self.pair_weights.data.iter()
            .chain(self.bin_weights.iter())
            .max_by_key(|&w| OrderedFloat(*w))
            .copied()
            .unwrap();

        info!(
            "rescaled weights to from [1.0, {:.3}] to [1.0, {:.3}] (x{:.3})",
            max_weight, new_max, rescale_factor
        );
    }

    pub fn get_pair_weight(&self, pk1: PItemKey, pk2: PItemKey) -> fsize {
        assert_ne!(pk1, pk2);
        let idx1 = self.pk_idx_map[pk1];
        let idx2 = self.pk_idx_map[pk2];

        self.pair_weights[(idx1, idx2)]
    }

    pub fn get_bin_weight(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];
        self.bin_weights[idx]
    }

    pub fn get_pair_overlap(&self, pk1: PItemKey, pk2: PItemKey) -> fsize {
        let idx1 = self.pk_idx_map[pk1];
        let idx2 = self.pk_idx_map[pk2];

        self.pair_overlap[(idx1, idx2)]
    }

    pub fn get_bin_overlap(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];
        self.bin_overlap[idx]
    }

    pub fn get_overlap(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];

        self.bin_overlap[idx] +
            self.pair_overlap.row(idx).iter().sum::<fsize>()
    }

    pub fn get_weighted_overlap(&self, pk: PItemKey) -> fsize {
        let idx = self.pk_idx_map[pk];

        let w_bin_overlap = self.bin_overlap[idx] * self.bin_weights[idx];
        let w_pair_overlap = self.pair_overlap.row(idx).iter()
            .zip(self.pair_weights.row(idx).iter())
            .map(|(&o, &w)| o * w)
            .sum::<fsize>();

        w_bin_overlap + w_pair_overlap
    }

    pub fn get_total_overlap(&self) -> fsize {
        self.bin_overlap.iter().sum::<fsize>() +
            self.pair_overlap.data.iter().sum::<fsize>()
    }

    pub fn get_total_weighted_overlap(&self) -> fsize {
        self.bin_overlap.iter()
            .zip(self.bin_weights.iter())
            .map(|(&o, &w)| o * w)
            .sum::<fsize>() +
            self.pair_overlap.data.iter()
                .zip(self.pair_weights.data.iter())
                .map(|(&o, &w)| o * w)
                .sum::<fsize>()
    }

    pub fn n_overlapping_items(&self) -> usize {
        (0..self.capacity).filter(|&idx| {
            self.pair_overlap.row(idx).iter().any(|&o| o > 0.0) || self.bin_overlap[idx] > 0.0
        }).count()
    }

    pub fn set_pair_weight(&mut self, pk1: PItemKey, pk2: PItemKey, weight: fsize) {
        assert_ne!(pk1, pk2);
        let idx1 = self.pk_idx_map[pk1];
        let idx2 = self.pk_idx_map[pk2];

        self.pair_weights[(idx1, idx2)] = weight;
        self.pair_weights[(idx2, idx1)] = weight;
    }

    pub fn set_jumped(&mut self, pk: PItemKey){
        let idx = self.pk_idx_map[pk];
        self.last_jump[idx] = self.weight_iter;
        debug!("jump registered at iter: {}", self.weight_iter);
    }

    pub fn is_on_jump_cooldown(&self, pk: PItemKey) -> bool {
        let idx = self.pk_idx_map[pk];
        let current_iter = self.weight_iter;
        let last_jump = self.last_jump[idx];

        current_iter - last_jump < self.jump_cooldown
    }
}

pub fn tracker_matches_layout(ot: &OverlapTracker, l: &Layout) -> bool {
    assert!(l.placed_items.keys().all(|k| ot.pk_idx_map.contains_key(k)));
    assert!(assert_matrix_symmetrical(&ot.pair_overlap));
    assert!(assert_matrix_symmetrical(&ot.pair_weights));

    for (pk1, pi1) in l.placed_items.iter() {
        let mut collisions = vec![];
        l.cde()
            .collect_poly_collisions(&pi1.shape, &[pi1.into()], &mut collisions);
        assert_eq!(ot.get_pair_overlap(pk1, pk1), 0.0);
        for (pk2, pi2) in l.placed_items.iter().filter(|(k, _)| *k != pk1) {
            let stored_overlap = ot.get_pair_overlap(pk1, pk2);
            match collisions.contains(&(pi2.into())) {
                true => {
                    let calc_overlap =
                        overlap_proxy::poly_overlap_proxy(&pi1.shape, &pi2.shape, l.bin.bbox());
                    let calc_overlap2 =
                        overlap_proxy::poly_overlap_proxy(&pi2.shape, &pi1.shape, l.bin.bbox());
                    if !approx_eq!(
                        fsize,
                        calc_overlap,
                        stored_overlap,
                        epsilon = 0.001 * stored_overlap
                    ) {
                        let mut opposite_collisions = vec![];
                        l.cde().collect_poly_collisions(
                            &pi2.shape,
                            &[pi2.into()],
                            &mut opposite_collisions,
                        );
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
                        }
                    }
                }
                false => {
                    if stored_overlap != 0.0 {
                        let calc_overlap =
                            overlap_proxy::poly_overlap_proxy(&pi1.shape, &pi2.shape, l.bin.bbox());
                        let mut opposite_collisions = vec![];
                        l.cde().collect_poly_collisions(
                            &pi2.shape,
                            &[pi2.into()],
                            &mut opposite_collisions,
                        );
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
