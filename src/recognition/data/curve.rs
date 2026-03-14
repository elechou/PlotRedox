use super::sampling::sample_points_from_cluster;

pub(super) fn sample_curve_points(
    pixels: &[(u32, u32)],
    point_count: usize,
    image_width: u32,
) -> Vec<(f32, f32)> {
    sample_points_from_cluster(pixels, point_count, image_width)
}
