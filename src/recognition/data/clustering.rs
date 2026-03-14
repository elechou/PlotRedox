use crate::state::{DataCurveMode, DataDetectionResult, DetectedColorGroup};

use super::super::pixels::{collect_masked_non_bg_pixels, color_distance_sq};
use super::super::spatial::split_into_connected_components;
use super::curve::sample_curve_points;

pub(super) fn analyze_mask_for_data(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
    tolerance: f32,
) -> DataDetectionResult {
    let pixel_colors = collect_masked_non_bg_pixels(rgba, mask, w, h, bg_color);

    if pixel_colors.is_empty() {
        return DataDetectionResult { groups: Vec::new() };
    }

    let mut centroids: Vec<[f32; 3]> = Vec::new();
    let mut cluster_pixels: Vec<Vec<(u32, u32)>> = Vec::new();

    for &(pixel, x, y) in &pixel_colors {
        let pf = [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32];

        let mut best_idx: Option<usize> = None;
        let mut best_dist = f32::MAX;
        for (i, centroid) in centroids.iter().enumerate() {
            let dist = color_distance_sq(
                pixel,
                [centroid[0] as u8, centroid[1] as u8, centroid[2] as u8],
            );
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }

        let tol_sq = tolerance * tolerance * 3.0;
        if let Some(idx) = best_idx {
            if best_dist < tol_sq {
                let n = cluster_pixels[idx].len() as f32;
                centroids[idx][0] = (centroids[idx][0] * n + pf[0]) / (n + 1.0);
                centroids[idx][1] = (centroids[idx][1] * n + pf[1]) / (n + 1.0);
                centroids[idx][2] = (centroids[idx][2] * n + pf[2]) / (n + 1.0);
                cluster_pixels[idx].push((x, y));
            } else {
                centroids.push(pf);
                cluster_pixels.push(vec![(x, y)]);
            }
        } else {
            centroids.push(pf);
            cluster_pixels.push(vec![(x, y)]);
        }
    }

    let mut groups = Vec::new();

    for (i, pixels) in cluster_pixels.into_iter().enumerate() {
        if pixels.len() < 5 {
            continue;
        }

        let filtered_pixels: Vec<(u32, u32)> = split_into_connected_components(&pixels, 1)
            .into_iter()
            .filter(|component| component.len() >= 3)
            .flatten()
            .collect();
        if filtered_pixels.len() < 5 {
            continue;
        }

        let avg_color = [
            centroids[i][0] as u8,
            centroids[i][1] as u8,
            centroids[i][2] as u8,
        ];

        let sampled = sample_curve_points(&filtered_pixels, 10, w);

        groups.push(DetectedColorGroup {
            color: avg_color,
            pixel_coords: filtered_pixels,
            curve_mode: DataCurveMode::Continuous,
            point_count: 10,
            sampled_points: sampled,
        });
    }

    groups.sort_by(|a, b| b.pixel_coords.len().cmp(&a.pixel_coords.len()));

    DataDetectionResult { groups }
}
