use jagua_rs::collision_detection::hpg::hazard_proximity_grid::HazardProximityGrid;
use jagua_rs::collision_detection::hpg::hpg_cell::HPGCell;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use ordered_float::OrderedFloat;

pub fn hpg_value(hpg: &HazardProximityGrid, shape: &SimplePolygon) -> fsize {

    //compute the total value of HPG grid within the expanded bbox of the shape.
    //the higher the hpg grid value for an (overlapping) placement the better.

    // let bbox = shape.bbox().scale(1.5);
    //
    // let rows = hpg.grid.rows_in_range(bbox.y_min..=bbox.y_max);
    // let cols = hpg.grid.cols_in_range(bbox.x_min..=bbox.x_max);
    //
    // let mut value = 0.0;
    //
    // for row in rows {
    //     //take a slice of the grid
    //     let slice = {
    //         let start_idx = row * hpg.grid.n_cols + cols.start();
    //         let end_idx = row * hpg.grid.n_cols + cols.end();
    //         &hpg.grid.cells[start_idx..=end_idx]
    //     };
    //     let row_val: fsize = slice.iter().flatten().map(|c| cell_value(c)).sum();
    //     value += row_val;
    // }
    //
    // value
    1.0
}

fn cell_value(c: &HPGCell) -> fsize {
    let uni_prox = c.uni_prox.0;
    uni_prox.powi(2)
}