use crate::state::{AxisDetectionResult, DataCurveMode, DataDetectionResult, DetectedColorGroup};
use std::collections::{HashMap, HashSet, VecDeque};

// ────────────────────────────────────────────────────────────────
//  Background Color Detection
// ────────────────────────────────────────────────────────────────

/// Detect the most common color in the image (quantized to reduce palette).
pub fn detect_background_color(rgba: &[u8], w: u32, h: u32) -> [u8; 3] {
    let mut histogram: HashMap<(u8, u8, u8), u32> = HashMap::new();
    let total = (w as usize) * (h as usize);

    for i in 0..total {
        let off = i * 4;
        if off + 2 >= rgba.len() {
            break;
        }
        let r = rgba[off] >> 5;
        let g = rgba[off + 1] >> 5;
        let b = rgba[off + 2] >> 5;
        *histogram.entry((r, g, b)).or_insert(0) += 1;
    }

    let (best_q, _) = histogram.iter().max_by_key(|&(_, &c)| c).unwrap_or((&(7, 7, 7), &0));

    [
        (best_q.0 << 5) | 0x10,
        (best_q.1 << 5) | 0x10,
        (best_q.2 << 5) | 0x10,
    ]
}

// ────────────────────────────────────────────────────────────────
//  Color Distance
// ────────────────────────────────────────────────────────────────

fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> f32 {
    let dr = a[0] as f32 - b[0] as f32;
    let dg = a[1] as f32 - b[1] as f32;
    let db = a[2] as f32 - b[2] as f32;
    dr * dr + dg * dg + db * db
}

fn is_bg_color(pixel: [u8; 3], bg: [u8; 3]) -> bool {
    color_distance_sq(pixel, bg) < 30.0 * 30.0 * 3.0
}

fn _is_similar_with_tolerance(a: [u8; 3], b: [u8; 3], tolerance: f32) -> bool {
    color_distance_sq(a, b) < tolerance * tolerance * 3.0
}

// ────────────────────────────────────────────────────────────────
//  Flood-Fill Connectivity Filter
// ────────────────────────────────────────────────────────────────

/// Filter axis-colored pixels to only those 8-connected to the main axis line,
/// within a maximum perpendicular distance. This removes text labels and other
/// disconnected features that share the axis color — even if they are connected
/// to the axis via tick marks (since text sits beyond `max_distance`).
///
/// - `axis_pixels`: all pixels matching the axis color
/// - `axis_line`: the row (for X-axis) or column (for Y-axis) of the detected axis line
/// - `is_horizontal`: true for X-axis (seed by row), false for Y-axis (seed by column)
/// - `line_thickness`: ±tolerance for "on the axis line" seed (typically 2)
/// - `max_distance`: maximum perpendicular distance from axis line to include
fn flood_fill_connected_to_axis(
    axis_pixels: &[(u32, u32)],
    axis_line: u32,
    is_horizontal: bool,
    line_thickness: u32,
    max_distance: u32,
) -> Vec<(u32, u32)> {
    // Build a set of all axis-colored pixel positions for O(1) lookup
    let pixel_set: HashSet<(u32, u32)> = axis_pixels.iter().copied().collect();

    // Seed: all axis-colored pixels that lie on/near the axis line
    let mut visited: HashSet<(u32, u32)> = HashSet::new();
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

    for &(x, y) in axis_pixels {
        let on_axis = if is_horizontal {
            (y as i32 - axis_line as i32).unsigned_abs() <= line_thickness
        } else {
            (x as i32 - axis_line as i32).unsigned_abs() <= line_thickness
        };
        if on_axis && visited.insert((x, y)) {
            queue.push_back((x, y));
        }
    }

    // BFS: expand 8-directionally within the axis-colored pixel set,
    // but ONLY to pixels within max_distance of the axis line.
    // This prevents the fill from walking through tick marks into text labels.
    while let Some((cx, cy)) = queue.pop_front() {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let np = (nx as u32, ny as u32);

                // Enforce max perpendicular distance from the axis line
                let perp_dist = if is_horizontal {
                    (ny - axis_line as i32).unsigned_abs()
                } else {
                    (nx - axis_line as i32).unsigned_abs()
                };
                if perp_dist > max_distance {
                    continue;
                }

                if pixel_set.contains(&np) && visited.insert(np) {
                    queue.push_back(np);
                }
            }
        }
    }

    visited.into_iter().collect()
}

// ────────────────────────────────────────────────────────────────
//  Axis Detection (Improved)
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

    // Step 1: Find the most common non-background color in masked region
    let mut color_counts: HashMap<(u8, u8, u8), u32> = HashMap::new();

    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask[idx] { continue; }
            let off = idx * 4;
            if off + 2 >= rgba.len() { continue; }
            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_bg_color(pixel, bg_color) { continue; }
            let qr = pixel[0] >> 6;
            let qg = pixel[1] >> 6;
            let qb = pixel[2] >> 6;
            *color_counts.entry((qr, qg, qb)).or_insert(0) += 1;
        }
    }

    let axis_color_q = color_counts
        .iter()
        .max_by_key(|&(_, &c)| c)
        .map(|(&k, _)| k);

    if axis_color_q.is_none() {
        return AxisDetectionResult {
            x_axis: None, y_axis: None,
            x_axis_pixels: Vec::new(), y_axis_pixels: Vec::new(),
            x_ticks: Vec::new(), y_ticks: Vec::new(),
        };
    }

    let axis_q = axis_color_q.unwrap();

    // Step 2: Collect all axis-colored pixels
    let mut axis_pixels: Vec<(u32, u32)> = Vec::new();
    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask[idx] { continue; }
            let off = idx * 4;
            if off + 2 >= rgba.len() { continue; }
            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_bg_color(pixel, bg_color) { continue; }
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
            x_axis: None, y_axis: None,
            x_axis_pixels: Vec::new(), y_axis_pixels: Vec::new(),
            x_ticks: Vec::new(), y_ticks: Vec::new(),
        };
    }

    // Step 3: Find the densest single row → actual X-axis line
    let mut row_pixel_counts: HashMap<u32, u32> = HashMap::new();
    let mut col_pixel_counts: HashMap<u32, u32> = HashMap::new();
    let mut row_xs: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut col_ys: HashMap<u32, Vec<u32>> = HashMap::new();

    for &(x, y) in &axis_pixels {
        *row_pixel_counts.entry(y).or_insert(0) += 1;
        *col_pixel_counts.entry(x).or_insert(0) += 1;
        row_xs.entry(y).or_default().push(x);
        col_ys.entry(x).or_default().push(y);
    }

    // Find the row with the most pixels → this IS the axis line
    let x_axis_row = row_pixel_counts.iter()
        .filter(|&(_, &count)| count >= 10)
        .max_by_key(|&(_, &count)| count)
        .map(|(&y, _)| y);

    let y_axis_col = col_pixel_counts.iter()
        .filter(|&(_, &count)| count >= 10)
        .max_by_key(|&(_, &count)| count)
        .map(|(&x, _)| x);

    // Step 4: Detect ticks — short perpendicular segments extending from the axis
    //         Use flood-fill connectivity to exclude disconnected text labels.
    let mut x_ticks: Vec<(f32, f32)> = Vec::new();
    let mut y_ticks: Vec<(f32, f32)> = Vec::new();
    let mut x_axis_pixels: Vec<(u32, u32)> = Vec::new();
    let mut y_axis_pixels: Vec<(u32, u32)> = Vec::new();

    if let Some(axis_y) = x_axis_row {
        // Filter to only pixels connected to the X-axis line,
        // bounded by tick_max_length to exclude text labels beyond ticks
        let tick_max_length = 30u32;
        let connected = flood_fill_connected_to_axis(
            &axis_pixels, axis_y, true, 2, tick_max_length,
        );

        // Collect all connected pixels on or near this row (±2px for line thickness)
        for &(px, py) in &connected {
            if (py as i32 - axis_y as i32).unsigned_abs() <= 2 {
                x_axis_pixels.push((px, py));
            }
        }

        // Find ticks using only connected pixels
        let tick_min_length = 3u32;
        let tick_max_length = 30u32;

        let mut col_runs: HashMap<u32, (u32, u32)> = HashMap::new(); // col -> (min_y, max_y)
        for &(px, py) in &connected {
            let entry = col_runs.entry(px).or_insert((py, py));
            entry.0 = entry.0.min(py);
            entry.1 = entry.1.max(py);
        }

        for (&col_x, &(min_y, max_y)) in &col_runs {
            // Check if this column crosses the axis line
            if min_y <= axis_y && max_y >= axis_y {
                let extension_down = max_y.saturating_sub(axis_y);
                let extension_up = axis_y.saturating_sub(min_y);
                let max_ext = extension_down.max(extension_up);

                // It's a tick if it extends beyond the line thickness but not too far
                if max_ext >= tick_min_length && max_ext <= tick_max_length {
                    // Tick position: on the axis line at this column
                    x_ticks.push((col_x as f32, axis_y as f32));
                    // Also add tick pixels to the highlight set
                    for &(px, py) in &connected {
                        if px == col_x && (py as i32 - axis_y as i32).unsigned_abs() <= max_ext {
                            if !x_axis_pixels.contains(&(px, py)) {
                                x_axis_pixels.push((px, py));
                            }
                        }
                    }
                }
            }
        }

        // Sort ticks by x coordinate
        x_ticks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    if let Some(axis_x) = y_axis_col {
        // Filter to only pixels connected to the Y-axis line,
        // bounded by tick_max_length to exclude text labels beyond ticks
        let tick_max_length = 30u32;
        let connected = flood_fill_connected_to_axis(
            &axis_pixels, axis_x, false, 2, tick_max_length,
        );

        // Collect all connected pixels on or near this column (±2px for line thickness)
        for &(px, py) in &connected {
            if (px as i32 - axis_x as i32).unsigned_abs() <= 2 {
                y_axis_pixels.push((px, py));
            }
        }

        // Find ticks using only connected pixels
        let tick_min_length = 3u32;
        let tick_max_length = 30u32;

        let mut row_runs: HashMap<u32, (u32, u32)> = HashMap::new(); // row -> (min_x, max_x)
        for &(px, py) in &connected {
            let entry = row_runs.entry(py).or_insert((px, px));
            entry.0 = entry.0.min(px);
            entry.1 = entry.1.max(px);
        }

        for (&row_y, &(min_x, max_x)) in &row_runs {
            if min_x <= axis_x && max_x >= axis_x {
                let extension_right = max_x.saturating_sub(axis_x);
                let extension_left = axis_x.saturating_sub(min_x);
                let max_ext = extension_right.max(extension_left);

                if max_ext >= tick_min_length && max_ext <= tick_max_length {
                    y_ticks.push((axis_x as f32, row_y as f32));
                    for &(px, py) in &connected {
                        if py == row_y && (px as i32 - axis_x as i32).unsigned_abs() <= max_ext {
                            if !y_axis_pixels.contains(&(px, py)) {
                                y_axis_pixels.push((px, py));
                            }
                        }
                    }
                }
            }
        }

        y_ticks.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Step 5: Endpoints = outermost ticks (or axis line ends if no ticks found)
    let x_axis = if !x_ticks.is_empty() {
        Some((*x_ticks.first().unwrap(), *x_ticks.last().unwrap()))
    } else if let Some(axis_y) = x_axis_row {
        if let Some(xs) = row_xs.get(&axis_y) {
            let min_x = *xs.iter().min().unwrap();
            let max_x = *xs.iter().max().unwrap();
            Some(((min_x as f32, axis_y as f32), (max_x as f32, axis_y as f32)))
        } else { None }
    } else { None };

    let y_axis = if !y_ticks.is_empty() {
        Some((*y_ticks.first().unwrap(), *y_ticks.last().unwrap()))
    } else if let Some(axis_x) = y_axis_col {
        if let Some(ys) = col_ys.get(&axis_x) {
            let min_y = *ys.iter().min().unwrap();
            let max_y = *ys.iter().max().unwrap();
            Some(((axis_x as f32, min_y as f32), (axis_x as f32, max_y as f32)))
        } else { None }
    } else { None };

    // Add remaining unclassified axis pixels
    let remaining: Vec<(u32, u32)> = axis_pixels
        .iter()
        .filter(|p| !x_axis_pixels.contains(p) && !y_axis_pixels.contains(p))
        .copied()
        .collect();

    for (px, py) in remaining {
        let dist_to_x = x_axis_row.map_or(f32::MAX, |ay| (py as f32 - ay as f32).abs());
        let dist_to_y = y_axis_col.map_or(f32::MAX, |ax| (px as f32 - ax as f32).abs());
        if dist_to_x < dist_to_y {
            x_axis_pixels.push((px, py));
        } else {
            y_axis_pixels.push((px, py));
        }
    }

    AxisDetectionResult {
        x_axis,
        y_axis,
        x_axis_pixels,
        y_axis_pixels,
        x_ticks,
        y_ticks,
    }
}

// ────────────────────────────────────────────────────────────────
//  Data Recognition: Color Clustering with Tolerance
// ────────────────────────────────────────────────────────────────

pub fn analyze_mask_for_data(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
    tolerance: f32,
) -> DataDetectionResult {
    let w_us = w as usize;
    let h_us = h as usize;

    // Step 1: Collect all non-background pixels in masked region
    let mut pixel_colors: Vec<([u8; 3], u32, u32)> = Vec::new();

    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask[idx] { continue; }
            let off = idx * 4;
            if off + 2 >= rgba.len() { continue; }
            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_bg_color(pixel, bg_color) { continue; }
            pixel_colors.push((pixel, x as u32, y as u32));
        }
    }

    if pixel_colors.is_empty() {
        return DataDetectionResult { groups: Vec::new() };
    }

    // Step 2: Cluster using user-adjustable tolerance
    // Use a greedy centroid-based clustering approach
    let mut centroids: Vec<[f32; 3]> = Vec::new();
    let mut cluster_pixels: Vec<Vec<(u32, u32)>> = Vec::new();

    for &(pixel, x, y) in &pixel_colors {
        let pf = [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32];

        // Find nearest centroid within tolerance
        let mut best_idx: Option<usize> = None;
        let mut best_dist = f32::MAX;
        for (i, centroid) in centroids.iter().enumerate() {
            let dr = pf[0] - centroid[0];
            let dg = pf[1] - centroid[1];
            let db = pf[2] - centroid[2];
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }

        let tol_sq = tolerance * tolerance * 3.0;
        if let Some(idx) = best_idx {
            if best_dist < tol_sq {
                // Add to existing cluster
                let n = cluster_pixels[idx].len() as f32;
                // Update running centroid
                centroids[idx][0] = (centroids[idx][0] * n + pf[0]) / (n + 1.0);
                centroids[idx][1] = (centroids[idx][1] * n + pf[1]) / (n + 1.0);
                centroids[idx][2] = (centroids[idx][2] * n + pf[2]) / (n + 1.0);
                cluster_pixels[idx].push((x, y));
            } else {
                // New cluster
                centroids.push(pf);
                cluster_pixels.push(vec![(x, y)]);
            }
        } else {
            centroids.push(pf);
            cluster_pixels.push(vec![(x, y)]);
        }
    }

    // Step 3: Build groups from clusters (skip noise clusters < 5 pixels)
    let mut groups: Vec<DetectedColorGroup> = Vec::new();

    for (i, pixels) in cluster_pixels.into_iter().enumerate() {
        if pixels.len() < 5 { continue; }

        let avg_color = [
            centroids[i][0] as u8,
            centroids[i][1] as u8,
            centroids[i][2] as u8,
        ];

        let sampled = sample_points_arc_length(&pixels, 10, w);

        groups.push(DetectedColorGroup {
            color: avg_color,
            pixel_coords: pixels,
            curve_mode: DataCurveMode::Continuous,
            point_count: 10,
            sampled_points: sampled,
        });
    }

    groups.sort_by(|a, b| b.pixel_coords.len().cmp(&a.pixel_coords.len()));

    DataDetectionResult { groups }
}

// ────────────────────────────────────────────────────────────────
//  Arc-Length Point Sampling (Multi-Segment)
// ────────────────────────────────────────────────────────────────

/// Sample N points along a pixel cluster using arc-length parameterization.
/// Handles non-function curves (circles, hyperbolas) correctly.
/// Supports multi-segment curves (occluded curves with gaps).
pub fn sample_points_from_cluster(
    pixels: &[(u32, u32)],
    n: usize,
    w: u32,
) -> Vec<(f32, f32)> {
    sample_points_arc_length(pixels, n, w)
}

fn sample_points_arc_length(
    pixels: &[(u32, u32)],
    n: usize,
    _w: u32,
) -> Vec<(f32, f32)> {
    if pixels.is_empty() || n == 0 {
        return Vec::new();
    }

    // Build all connected chains (handles gaps from occlusion)
    let chains = build_pixel_chains(pixels);

    if chains.is_empty() {
        return Vec::new();
    }

    // Compute arc-lengths for each chain
    let mut chain_lengths: Vec<f32> = Vec::new();
    let mut chain_arc_data: Vec<Vec<f32>> = Vec::new(); // cumulative arc-lengths per chain

    for chain in &chains {
        let mut arcs: Vec<f32> = vec![0.0];
        for i in 1..chain.len() {
            let dx = chain[i].0 as f32 - chain[i - 1].0 as f32;
            let dy = chain[i].1 as f32 - chain[i - 1].1 as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            arcs.push(arcs[i - 1] + dist);
        }
        let total = *arcs.last().unwrap_or(&0.0);
        chain_lengths.push(total);
        chain_arc_data.push(arcs);
    }

    let grand_total: f32 = chain_lengths.iter().sum();
    if grand_total < 1.0 {
        // All chains are trivially short
        return chains.iter()
            .flat_map(|c| c.iter().map(|&(x, y)| (x as f32, y as f32)))
            .take(n)
            .collect();
    }

    // Total points in all chains combined
    let total_points_available: usize = chains.iter().map(|c| c.len()).sum();
    if total_points_available <= n {
        return chains.iter()
            .flat_map(|c| c.iter().map(|&(x, y)| (x as f32, y as f32)))
            .collect();
    }

    // Distribute N sample points across chains proportionally to arc-length
    let mut points_per_chain: Vec<usize> = Vec::new();
    let mut allocated = 0usize;

    for (i, &len) in chain_lengths.iter().enumerate() {
        let share = if grand_total > 0.0 {
            (len / grand_total * n as f32).round() as usize
        } else {
            0
        };
        // Ensure at least 1 point per chain (if chain is non-trivial)
        let share = if chains[i].len() >= 2 { share.max(1) } else { share };
        points_per_chain.push(share);
        allocated += share;
    }

    // Adjust for rounding: add/remove from the longest chain
    if allocated != n {
        if let Some(longest_idx) = chain_lengths
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
        {
            if allocated > n {
                let excess = allocated - n;
                points_per_chain[longest_idx] =
                    points_per_chain[longest_idx].saturating_sub(excess);
            } else {
                points_per_chain[longest_idx] += n - allocated;
            }
        }
    }

    // Sample each chain independently
    let mut sampled = Vec::with_capacity(n);

    for (chain_idx, chain) in chains.iter().enumerate() {
        let cn = points_per_chain[chain_idx];
        if cn == 0 || chain.is_empty() {
            continue;
        }

        let arcs = &chain_arc_data[chain_idx];
        let total_length = chain_lengths[chain_idx];

        if chain.len() <= cn {
            sampled.extend(chain.iter().map(|&(x, y)| (x as f32, y as f32)));
            continue;
        }

        if total_length < 1.0 {
            sampled.push((chain[0].0 as f32, chain[0].1 as f32));
            continue;
        }

        for i in 0..cn {
            let target = if cn == 1 {
                total_length / 2.0
            } else {
                (i as f32) * total_length / ((cn - 1) as f32)
            };

            // Binary search for the segment containing this arc-length
            let seg = match arcs.binary_search_by(|v| {
                v.partial_cmp(&target).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(idx) => idx,
                Err(idx) => idx.saturating_sub(1),
            };
            let seg = seg.min(chain.len() - 1);

            // Interpolate within the segment
            if seg + 1 < chain.len() {
                let seg_start = arcs[seg];
                let seg_end = arcs[seg + 1];
                let seg_len = seg_end - seg_start;
                let t = if seg_len > 0.0 { (target - seg_start) / seg_len } else { 0.0 };
                let x = chain[seg].0 as f32
                    + t * (chain[seg + 1].0 as f32 - chain[seg].0 as f32);
                let y = chain[seg].1 as f32
                    + t * (chain[seg + 1].1 as f32 - chain[seg].1 as f32);
                sampled.push((x, y));
            } else {
                sampled.push((chain[seg].0 as f32, chain[seg].1 as f32));
            }
        }
    }

    sampled
}

/// Build ordered chains of points by nearest-neighbor walking.
/// Returns ALL segments (handles gaps from occlusion).
/// Each gap > threshold starts a new chain instead of stopping.
fn build_pixel_chains(pixels: &[(u32, u32)]) -> Vec<Vec<(u32, u32)>> {
    if pixels.is_empty() {
        return Vec::new();
    }
    if pixels.len() <= 2 {
        return vec![pixels.to_vec()];
    }

    // For large pixel sets, thin to medial axis first (take median Y per X column)
    let mut by_x: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(x, y) in pixels {
        by_x.entry(x).or_default().push(y);
    }

    let mut thin_points: Vec<(u32, u32)> = Vec::new();
    for (&x, ys) in &by_x {
        let mut ys_sorted = ys.clone();
        ys_sorted.sort();
        let median_y = ys_sorted[ys_sorted.len() / 2];
        thin_points.push((x, median_y));
    }

    if thin_points.len() <= 2 {
        thin_points.sort_by_key(|p| p.0);
        return vec![thin_points];
    }

    // Nearest-neighbor chain starting from the leftmost point
    thin_points.sort_by_key(|p| p.0);
    let mut used = vec![false; thin_points.len()];
    let mut chains: Vec<Vec<(u32, u32)>> = Vec::new();

    // Start from the point with the smallest x
    let mut current_chain: Vec<(u32, u32)> = Vec::new();
    let current = 0;
    current_chain.push(thin_points[current]);
    used[current] = true;

    let gap_threshold_sq = 100.0 * 100.0;

    for _ in 1..thin_points.len() {
        let mut best_idx = None;
        let mut best_dist = f64::MAX;

        for (j, &pt) in thin_points.iter().enumerate() {
            if used[j] { continue; }
            let dx = pt.0 as f64 - current_chain.last().unwrap().0 as f64;
            let dy = pt.1 as f64 - current_chain.last().unwrap().1 as f64;
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(j);
            }
        }

        if let Some(idx) = best_idx {
            if best_dist > gap_threshold_sq {
                // Gap detected: save current chain and start a new one
                if current_chain.len() >= 2 {
                    chains.push(std::mem::take(&mut current_chain));
                } else {
                    current_chain.clear();
                }
            }
            current_chain.push(thin_points[idx]);
            used[idx] = true;
        } else {
            break;
        }
    }

    // Don't forget the last chain
    if current_chain.len() >= 2 {
        chains.push(current_chain);
    }

    // If nothing was produced (e.g. all single-point chains), fall back
    if chains.is_empty() {
        thin_points.sort_by_key(|p| p.0);
        return vec![thin_points];
    }

    chains
}
