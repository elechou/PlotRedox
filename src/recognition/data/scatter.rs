use super::sampling::sample_points_from_cluster;

pub(super) fn sample_scatter_points(pixels: &[(u32, u32)], image_width: u32) -> Vec<(f32, f32)> {
    sample_points_from_cluster(pixels, pixels.len(), image_width)
}
