use std::cmp::Ordering;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::{Transformable, TransformableFrom};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use jagua_rs::geometry::transformation::Transformation;
use jagua_rs::util::fpa::FPA;
use crate::overlap::overlap::{calculate_unweighted_overlap_shape, calculate_weighted_overlap};
use crate::overlap::overlap_tracker::OverlapTracker;

pub struct SampleEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    current_pk: Option<PItemKey>,
    ot: &'a OverlapTracker,
    coll_buff: Vec<HazardEntity>,
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> SampleEvaluator<'a> {
    pub fn new(layout: &'a Layout, item: &'a Item, current_pk: Option<PItemKey>, ot: &'a OverlapTracker) -> Self {
        Self {
            layout,
            item,
            current_pk,
            ot,
            coll_buff: vec![],
            shape_buff: item.shape.as_ref().clone(),
            n_evals: 0,
        }
    }

    pub fn eval(&mut self, transf: impl Into<Transformation>) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();

        self.coll_buff.clear();
        self.shape_buff.transform_from(&self.item.shape, &transf.into());

        match self.current_pk {
            Some(current_pk) => {
                let current_pi = &self.layout.placed_items[current_pk];
                cde.collect_poly_collisions(&self.shape_buff, &[current_pi.into()], &mut self.coll_buff);
            }
            None => {
                cde.collect_poly_collisions(&self.shape_buff, &[], &mut self.coll_buff);
            }
        }

        if self.coll_buff.is_empty() {
            SampleEval::Valid(0.0)
        }
        else {
            let w_overlap = match self.current_pk {
                Some(current_pk) => {
                    calculate_weighted_overlap(self.layout, &self.shape_buff, current_pk, self.coll_buff.iter().cloned(), self.ot)
                }
                None => {
                    calculate_unweighted_overlap_shape(self.layout, &self.shape_buff, self.coll_buff.iter().cloned())
                }
            };
            SampleEval::Colliding(self.coll_buff.len(), w_overlap)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SampleEval{
    Colliding(usize, fsize),
    Valid(fsize)
}

impl PartialOrd for SampleEval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (SampleEval::Valid(s1), SampleEval::Valid(s2)) => FPA(*s1).partial_cmp(&FPA(*s2)),
            (SampleEval::Colliding(_, s1), SampleEval::Colliding(_, s2)) => FPA(*s1).partial_cmp(&FPA(*s2)),
            (SampleEval::Valid(_), _) => Some(Ordering::Less),
            (_, SampleEval::Valid(_)) => Some(Ordering::Greater),
        }
    }
}

impl Ord for SampleEval {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Eq for SampleEval {}