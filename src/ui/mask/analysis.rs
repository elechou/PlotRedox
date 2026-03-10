use crate::state::{AxisDetectionResult, DataCurveMode, DataDetectionResult, DetectedColorGroup};
use std::collections::HashMap;

// ────────────────────────────────────────────────────────────────
//  Background Color Detection
// ────────────────────────────────────────────────────────────────

/// Detect the most common color in the image (quantized to reduce palette).
/// Returns the dominant RGB color.
pub fn detect_background_color(rgba: &[u8], w: u32, h: u32) -> [u8; 3] {
    let mut histogram: HashMap<(u8, u8, u8), u32> = HashMap::new();
    let total = (w as usize) * (h as usize);

    for i in 0..total {
        let off = i * 4;
        if off + 2 >= rgba.len() {
            break;
        }
        // Quantize to 8-level bins (divide by 32) for color grouping
        let r = rgba[off] >> 5;
        let g = rgba[off + 1] >> 5;
        let b = rgba[off + 2] >> 5;
        *histogram.entry((r, g, b)).or_insert(0) += 1;
    }

    // Find the most common quantized color
    let (best_q, _) = histogram.iter().max_by_key(|&(_, &c)| c).unwrap_or((&(7, 7, 7), &0));

    // Map back to approximate RGB center of the bin
    [
        (best_q.0 << 5) | 0x10,
        (best_q.1 << 5) | 0x10,
        (best_q.2 << 5) | 0x10,
    ]
}

// ────────────────────────────────────────────────────────────────
//  Color Distance
// ────────────────────────────────────────────────────────────────

fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    (dr * dr + dg * dg + db * db) as u32
}

/// Threshold for considering two colors "similar" (~30 per channel)
const COLOR_SIMILARITY_THRESHOLD: u32 = 30 * 30 * 3;

fn is_similar_color(a: [u8; 3], b: [u8; 3]) -> bool {
    color_distance_sq(a, b) < COLOR_SIMILARITY_THRESHOLD
}

// ────────────────────────────────────────────────────────────────
//  Axis Detection within Masked Region
// ────────────────────────────────────────────────────────────────

pub fn analyze_mask_for_axes(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
) -> AxisDetectionResult {
    let w_us = w as usize;
    let h_us = h as usize;

    // Step 1: Find the "second color" — most common non-background color in masked region
    let mut color_counts: HashMap<(u8, u8, u8), u32> = HashMap::new();

    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask[idx] {
                continue;
            }
            let off = idx * 4;
            if off + 2 >= rgba.len() {
                continue;
            }
            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_similar_color(pixel, bg_color) {
                continue;
            }
            // Quantize to 4-level bins for grouping
            let qr = pixel[0] >> 6;
            let qg = pixel[1] >> 6;
            let qb = pixel[2] >> 6;
            *color_counts.entry((qr, qg, qb)).or_insert(0) += 1;
        }
    }

    // Get the most common non-bg quantized color
    let axis_color_q = color_counts
        .iter()
        .max_by_key(|&(_, &c)| c)
        .map(|(&k, _)| k);

    if axis_color_q.is_none() {
        return AxisDetectionResult {
            x_axis: None,
            y_axis: None,
            x_axis_pixels: Vec::new(),
            y_axis_pixels: Vec::new(),
        };
    }

    let axis_q = axis_color_q.unwrap();
    let _axis_color_center = [
        (axis_q.0 << 6) | 0x20,
        (axis_q.1 << 6) | 0x20,
        (axis_q.2 << 6) | 0x20,
    ];

    // Step 2: Collect all axis-colored pixels in masked region
    let mut axis_pixels: Vec<(u32, u32)> = Vec::new();
    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask[idx] {
                continue;
            }
            let off = idx * 4;
            if off + 2 >= rgba.len() {
                continue;
            }
            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_similar_color(pixel, bg_color) {
                continue;
            }
            // Check if this pixel matches the axis color (quantized)
            let qr = pixel[0] >> 6;
            let qg = pixel[1] >> 6;
            let qb = pixel[2] >> 6;
            if (qr, qg, qb) == axis_q {
                axis_pixels.push((x as u32, y as u32));
            }
        }
    }

    if axis_pixels.is_empty() {
        return AxisDetectionResult {
            x_axis: None,
            y_axis: None,
            x_axis_pixels: Vec::new(),
            y_axis_pixels: Vec::new(),
        };
    }

    // Step 3: Detect horizontal and vertical line segments
    // Project pixels onto rows and columns, find rows/cols with many axis-colored pixels
    let mut row_counts: HashMap<u32, Vec<u32>> = HashMap::new(); // y -> list of x coords
    let mut col_counts: HashMap<u32, Vec<u32>> = HashMap::new(); // x -> list of y coords

    for &(x, y) in &axis_pixels {
        row_counts.entry(y).or_default().push(x);
        col_counts.entry(x).or_default().push(y);
    }

    // Find the densest horizontal band (potential X-axis)
    // Group adjacent rows with high pixel counts
    let x_axis_result = detect_axis_line(&row_counts, &axis_pixels, true, w, h);
    let y_axis_result = detect_axis_line(&col_counts, &axis_pixels, false, w, h);

    // Separate axis pixels into X and Y groups
    let mut x_axis_pixels = Vec::new();
    let mut y_axis_pixels = Vec::new();

    if let Some((start, end)) = &x_axis_result {
        // X-axis: pixels near the detected horizontal line
        let y_mid = (start.1 + end.1) / 2.0;
        let tolerance = 5.0;
        for &(px, py) in &axis_pixels {
            if (py as f32 - y_mid).abs() < tolerance {
                x_axis_pixels.push((px, py));
            }
        }
    }

    if let Some((start, end)) = &y_axis_result {
        // Y-axis: pixels near the detected vertical line
        let x_mid = (start.0 + end.0) / 2.0;
        let tolerance = 5.0;
        for &(px, py) in &axis_pixels {
            if (px as f32 - x_mid).abs() < tolerance {
                y_axis_pixels.push((px, py));
            }
        }
    }

    // If some pixels aren't classified, add them to the closer axis
    let remaining: Vec<(u32, u32)> = axis_pixels
        .iter()
        .filter(|p| !x_axis_pixels.contains(p) && !y_axis_pixels.contains(p))
        .copied()
        .collect();

    for (px, py) in remaining {
        let dist_to_x = if let Some((s, e)) = &x_axis_result {
            let y_mid = (s.1 + e.1) / 2.0;
            (py as f32 - y_mid).abs()
        } else {
            f32::MAX
        };
        let dist_to_y = if let Some((s, e)) = &y_axis_result {
            let x_mid = (s.0 + e.0) / 2.0;
            (px as f32 - x_mid).abs()
        } else {
            f32::MAX
        };

        if dist_to_x < dist_to_y {
            x_axis_pixels.push((px, py));
        } else {
            y_axis_pixels.push((px, py));
        }
    }

    AxisDetectionResult {
        x_axis: x_axis_result,
        y_axis: y_axis_result,
        x_axis_pixels,
        y_axis_pixels,
    }
}

/// Detect an axis line from row/column projection data.
/// For horizontal (is_row=true): finds the densest row band, then finds endpoints.
/// For vertical (is_row=false): finds the densest column band, then finds endpoints.
fn detect_axis_line(
    line_counts: &HashMap<u32, Vec<u32>>,
    _all_pixels: &[(u32, u32)],
    is_row: bool,
    _w: u32,
    _h: u32,
) -> Option<((f32, f32), (f32, f32))> {
    if line_counts.is_empty() {
        return None;
    }

    // Find the line (row or col) with the most pixels — this is likely the axis
    let min_pixel_count = 10; // Need at least 10 pixels to be considered an axis

    // Group adjacent lines into bands
    let mut lines_sorted: Vec<(u32, usize)> = line_counts
        .iter()
        .map(|(&k, v)| (k, v.len()))
        .filter(|&(_, count)| count >= 3)
        .collect();
    lines_sorted.sort_by_key(|&(k, _)| k);

    if lines_sorted.is_empty() {
        return None;
    }

    // Find best band of adjacent lines
    let mut best_band_start = 0;
    let mut best_band_end = 0;
    let mut best_band_total = 0usize;
    let mut band_start = 0;

    for i in 1..=lines_sorted.len() {
        let should_break = i == lines_sorted.len()
            || lines_sorted[i].0 > lines_sorted[i - 1].0 + 3; // Allow 3px gap

        if should_break {
            let total: usize = (band_start..i).map(|j| lines_sorted[j].1).sum();
            if total > best_band_total {
                best_band_total = total;
                best_band_start = band_start;
                best_band_end = i - 1;
            }
            band_start = i;
        }
    }

    if best_band_total < min_pixel_count {
        return None;
    }

    // The axis line position is the median of the band
    let band_min = lines_sorted[best_band_start].0;
    let band_max = lines_sorted[best_band_end].0;
    let axis_pos = (band_min + band_max) as f32 / 2.0;

    // Collect all cross-axis positions for pixels in this band
    let mut cross_positions: Vec<u32> = Vec::new();
    for &(line_idx, _) in &lines_sorted[best_band_start..=best_band_end] {
        if let Some(positions) = line_counts.get(&line_idx) {
            cross_positions.extend(positions);
        }
    }
    cross_positions.sort();

    if cross_positions.is_empty() {
        return None;
    }

    // Find the endpoints: look for tick marks at the extremes
    // The axis endpoints are the min and max cross-positions
    // But we should look for "tick" patterns — short perpendicular segments
    let min_cross = *cross_positions.first().unwrap() as f32;
    let max_cross = *cross_positions.last().unwrap() as f32;

    if is_row {
        // Horizontal axis: cross positions are X, axis_pos is Y
        Some(((min_cross, axis_pos), (max_cross, axis_pos)))
    } else {
        // Vertical axis: cross positions are Y, axis_pos is X
        Some(((axis_pos, min_cross), (axis_pos, max_cross)))
    }
}

// ────────────────────────────────────────────────────────────────
//  Data Recognition: Color Clustering within Masked Region
// ────────────────────────────────────────────────────────────────

pub fn analyze_mask_for_data(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
) -> DataDetectionResult {
    let w_us = w as usize;
    let h_us = h as usize;

    // Step 1: Collect all non-background pixels in masked region
    let mut pixel_colors: Vec<([u8; 3], u32, u32)> = Vec::new(); // (color, x, y)

    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask[idx] {
                continue;
            }
            let off = idx * 4;
            if off + 2 >= rgba.len() {
                continue;
            }
            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_similar_color(pixel, bg_color) {
                continue;
            }
            pixel_colors.push((pixel, x as u32, y as u32));
        }
    }

    if pixel_colors.is_empty() {
        return DataDetectionResult {
            groups: Vec::new(),
        };
    }

    // Step 2: Cluster by quantized color (4-level bins)
    let mut clusters: HashMap<(u8, u8, u8), Vec<(u32, u32)>> = HashMap::new();

    for &(pixel, x, y) in &pixel_colors {
        let qr = pixel[0] >> 6;
        let qg = pixel[1] >> 6;
        let qb = pixel[2] >> 6;
        clusters.entry((qr, qg, qb)).or_default().push((x, y));
    }

    // Step 3: Merge similar quantized clusters
    let mut merged_groups: Vec<DetectedColorGroup> = Vec::new();
    let mut processed: Vec<bool> = vec![false; clusters.len()];
    let cluster_keys: Vec<(u8, u8, u8)> = clusters.keys().copied().collect();

    for i in 0..cluster_keys.len() {
        if processed[i] {
            continue;
        }
        let key_i = cluster_keys[i];
        let center_i = [
            (key_i.0 << 6) | 0x20,
            (key_i.1 << 6) | 0x20,
            (key_i.2 << 6) | 0x20,
        ];

        let mut merged_pixels: Vec<(u32, u32)> = clusters[&key_i].clone();
        processed[i] = true;

        // Merge nearby clusters
        for j in (i + 1)..cluster_keys.len() {
            if processed[j] {
                continue;
            }
            let key_j = cluster_keys[j];
            let center_j = [
                (key_j.0 << 6) | 0x20,
                (key_j.1 << 6) | 0x20,
                (key_j.2 << 6) | 0x20,
            ];
            if is_similar_color(center_i, center_j) {
                merged_pixels.extend(clusters[&key_j].iter());
                processed[j] = true;
            }
        }

        // Skip very small clusters (noise)
        if merged_pixels.len() < 5 {
            continue;
        }

        // Compute average color of merged pixels
        let mut sum_r: u64 = 0;
        let mut sum_g: u64 = 0;
        let mut sum_b: u64 = 0;
        for &(x, y) in &merged_pixels {
            let off = ((y as usize) * w_us + (x as usize)) * 4;
            if off + 2 < rgba.len() {
                sum_r += rgba[off] as u64;
                sum_g += rgba[off + 1] as u64;
                sum_b += rgba[off + 2] as u64;
            }
        }
        let n = merged_pixels.len() as u64;
        let avg_color = [
            (sum_r / n) as u8,
            (sum_g / n) as u8,
            (sum_b / n) as u8,
        ];

        // Generate initial sampled points
        let sampled = sample_points_from_cluster(&merged_pixels, 10, w);

        merged_groups.push(DetectedColorGroup {
            color: avg_color,
            pixel_coords: merged_pixels,
            curve_mode: DataCurveMode::Continuous,
            point_count: 10,
            sampled_points: sampled,
        });
    }

    // Sort groups by pixel count (largest first)
    merged_groups.sort_by(|a, b| b.pixel_coords.len().cmp(&a.pixel_coords.len()));

    DataDetectionResult {
        groups: merged_groups,
    }
}

/// Sample N evenly-spaced points along a cluster of pixels.
/// For continuous curves: sorts by X, then picks N evenly spaced median-Y points.
/// For scatter: just picks the centroid of sub-clusters.
pub fn sample_points_from_cluster(
    pixels: &[(u32, u32)],
    n: usize,
    _w: u32,
) -> Vec<(f32, f32)> {
    if pixels.is_empty() || n == 0 {
        return Vec::new();
    }

    // Group pixels by X coordinate
    let mut by_x: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(x, y) in pixels {
        by_x.entry(x).or_default().push(y);
    }

    // Sort X values
    let mut x_vals: Vec<u32> = by_x.keys().copied().collect();
    x_vals.sort();

    if x_vals.is_empty() {
        return Vec::new();
    }

    // For each X, compute median Y
    let mut curve_points: Vec<(f32, f32)> = Vec::new();
    for &x in &x_vals {
        if let Some(ys) = by_x.get(&x) {
            let mut ys_sorted = ys.clone();
            ys_sorted.sort();
            let median_y = ys_sorted[ys_sorted.len() / 2];
            curve_points.push((x as f32, median_y as f32));
        }
    }

    if curve_points.len() <= n {
        return curve_points;
    }

    // Pick N evenly spaced points
    let mut sampled = Vec::with_capacity(n);
    for i in 0..n {
        let idx = if n == 1 {
            curve_points.len() / 2
        } else {
            i * (curve_points.len() - 1) / (n - 1)
        };
        sampled.push(curve_points[idx]);
    }

    sampled
}
