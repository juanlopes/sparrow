use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::{fsize, PI};

mod best_samples;
mod coord_descent;
pub mod search;
mod uniform_sampler;

pub fn dtransfs_are_similar(
    dt1: DTransformation,
    dt2: DTransformation,
    x_threshold: fsize,
    y_threshold: fsize,
) -> bool {
    let x_diff = fsize::abs(dt1.translation().0 - dt2.translation().0);
    let y_diff = fsize::abs(dt1.translation().1 - dt2.translation().1);

    if x_diff < x_threshold && y_diff < y_threshold {
        let r1 = dt1.rotation() % 2.0 * PI;
        let r2 = dt2.rotation() % 2.0 * PI;
        let angle_diff = fsize::abs(r1 - r2);
        angle_diff < (1.0 as fsize).to_radians()
    } else {
        false
    }
}
