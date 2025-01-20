use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use crate::overlap::overlap_proxy::{poly_overlap_proxy, bin_overlap_proxy};
use crate::overlap::overlap_tracker_original::OverlapTracker;

pub fn calculate_weighted_overlap<I>(l: &Layout, s: &SimplePolygon, ref_pk: PItemKey, overlapping: I, ot: &OverlapTracker) -> fsize
where I : Iterator<Item=HazardEntity> {
    overlapping.map(|haz| {
        match haz {
            HazardEntity::PlacedItem { .. } => {
                let other_pk = l.hazard_to_p_item_key(&haz).unwrap();
                let other_shape = &l.placed_items[other_pk].shape;
                let overlap = poly_overlap_proxy(s, other_shape, l.bin.bbox());
                let weight = ot.get_pair_weight(ref_pk, other_pk);
                overlap * weight
            }
            HazardEntity::BinExterior => {
                panic!();
                let overlap = bin_overlap_proxy(s, l.bin.bbox());
                let weight = ot.get_bin_weight(ref_pk);
                overlap * weight
            }
            _ => unimplemented!("unsupported hazard entity")
        }
    }).sum()
}

pub fn calculate_unweighted_overlap_shape<I>(l: &Layout, s: &SimplePolygon, overlapping: I) -> fsize
where I : Iterator<Item=HazardEntity> {
    overlapping.map(|haz| {
        match haz {
            HazardEntity::PlacedItem { .. } => {
                let other_pk = l.hazard_to_p_item_key(&haz).unwrap();
                let other_shape = &l.placed_items[other_pk].shape;
                poly_overlap_proxy(s, other_shape, l.bin.bbox())
            }
            HazardEntity::BinExterior => {
                bin_overlap_proxy(s, l.bin.bbox())
            }
            _ => unimplemented!("unsupported hazard entity")
        }
    }).sum()
}