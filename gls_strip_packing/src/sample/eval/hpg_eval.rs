use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::collision_detection::hpg::hazard_proximity_grid::HazardProximityGrid;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use ordered_float::OrderedFloat;

pub fn hpg_value(hpg: &HazardProximityGrid, shape: &SimplePolygon, haz: Option<HazardEntity>) -> fsize {

    //compute the total value of HPG grid within the expanded bbox of the shape.
    //the higher the hpg grid value for an (overlapping) placement the better.

    //const PADDING: fsize = 0.01;

    //extend the shapes bbox by 5% of the hpg bbox
    // let bbox = AARectangle::new(
    //     shape.bbox().x_min - PADDING * hpg.bbox.width(),
    //     shape.bbox().y_min - PADDING * hpg.bbox.height(),
    //     shape.bbox().x_max + PADDING * hpg.bbox.width(),
    //     shape.bbox().y_max + PADDING * hpg.bbox.height(),
    // );
    //
    // let rows = hpg.grid.rows_in_range(bbox.y_min..=bbox.y_max);
    // let cols = hpg.grid.cols_in_range(bbox.x_min..=bbox.x_max);
    //
    // let mut max_haz_prox = 0.0;
    //
    // for row in rows {
    //     //take a slice of the grid
    //     let slice = {
    //         let start_idx = row * hpg.grid.n_cols + cols.start();
    //         let end_idx = row * hpg.grid.n_cols + cols.end();
    //         &hpg.grid.cells[start_idx..=end_idx]
    //     };
    //     let row_max_haz_prox: fsize = slice.iter().flatten()
    //         .map(|c| {
    //             if Some(c.uni_prox.1) == haz {
    //                 shape.poi.radius
    //             } else {
    //                 c.uni_prox.0
    //             }
    //         })
    //         .max_by_key(|d| OrderedFloat(*d))
    //         .unwrap();
    //     max_haz_prox = fsize::max(max_haz_prox, row_max_haz_prox);
    // }
    //
    // assert!(max_haz_prox >= 0.0);
    //
    // max_haz_prox.powi(2)
    1.0
}