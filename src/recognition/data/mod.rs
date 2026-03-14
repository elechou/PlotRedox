mod clustering;
mod curve;
mod sampling;
mod scatter;

use crate::state::{DataCurveMode, DataDetectionResult};

pub fn analyze_mask_for_data(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
    tolerance: f32,
) -> DataDetectionResult {
    clustering::analyze_mask_for_data(rgba, mask, w, h, bg_color, tolerance)
}

pub fn sample_points_for_mode(
    mode: DataCurveMode,
    pixels: &[(u32, u32)],
    point_count: usize,
    image_width: u32,
) -> Vec<(f32, f32)> {
    match mode {
        DataCurveMode::Continuous => curve::sample_curve_points(pixels, point_count, image_width),
        DataCurveMode::Scatter => scatter::sample_scatter_points(pixels, image_width),
    }
}
