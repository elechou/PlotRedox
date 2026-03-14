use crate::state::AxisDetectionResult;
use image::GrayImage;
use imageproc::hough::{detect_lines, LineDetectionOptions, PolarLine};
use super::is_bg_color;

// ────────────────────────────────────────────────────────────────
//  Axis Detection — Industrial Pipeline
//
//  1. Build binary foreground image from mask
//  2. Directional morphological opening to isolate H/V structures
//  3. Hough line detection (supports ±5° tilt)
//  4. 1D perpendicular-extent profiling for tick extraction
//  5. Periodicity-based tick filtering
//  6. Pixel collection for UI highlighting
// ────────────────────────────────────────────────────────────────

/// Internal representation of a detected axis line.
/// Line equation: x·cos(θ) + y·sin(θ) = r
#[derive(Clone, Copy)]
struct AxisLine {
    cos_t: f32,
    sin_t: f32,
    r: f32,
}

impl AxisLine {
    fn from_polar(pl: &PolarLine) -> Self {
        let theta = (pl.angle_in_degrees as f32).to_radians();
        let (sin_t, cos_t) = theta.sin_cos();
        Self { cos_t, sin_t, r: pl.r }
    }

    /// Signed perpendicular distance from point to line.
    #[inline]
    fn perp_dist(&self, x: f32, y: f32) -> f32 {
        x * self.cos_t + y * self.sin_t - self.r
    }

    /// Project point onto line, returning parameter along tangent (−sin θ, cos θ).
    #[inline]
    fn project(&self, x: f32, y: f32) -> f32 {
        -x * self.sin_t + y * self.cos_t
    }

    /// Convert tangent parameter back to (x, y) on the line.
    fn point_at(&self, t: f32) -> (f32, f32) {
        (
            self.r * self.cos_t - t * self.sin_t,
            self.r * self.sin_t + t * self.cos_t,
        )
    }
}

// ────────────────────────────────────────────────────────────────
//  Public API (signature unchanged)
// ────────────────────────────────────────────────────────────────

pub fn analyze_mask_for_axes(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
) -> AxisDetectionResult {
    let empty = AxisDetectionResult {
        x_axis: None,
        y_axis: None,
        x_axis_pixels: Vec::new(),
        y_axis_pixels: Vec::new(),
        x_ticks: Vec::new(),
        y_ticks: Vec::new(),
    };

    // Step 1: Binary foreground image (within mask only)
    let fg = build_foreground_image(rgba, mask, w, h, bg_color);
    
    let mut active_pixels = Vec::with_capacity(8192);
    let raw = fg.as_raw();
    let wu = w as usize;
    for y in 0..h {
        let row = y as usize * wu;
        for x in 0..w {
            if raw[row + x as usize] > 0 {
                active_pixels.push((x as f32, y as f32));
            }
        }
    }

    if active_pixels.len() < 20 {
        return empty;
    }

    // Step 2: Directional morphological opening
    let min_dim = w.min(h) as f32;
    let kernel_len = (min_dim * 0.04).max(20.0) as u32;
    let h_open = directional_open(&fg, kernel_len, 1); // horizontal structures survive
    let v_open = directional_open(&fg, 1, kernel_len); // vertical structures survive

    // Step 3: Hough line detection
    let x_line = detect_best_line(&h_open, &active_pixels, true);
    let y_line = detect_best_line(&v_open, &active_pixels, false);
    if x_line.is_none() && y_line.is_none() {
        return empty;
    }

    // Step 4: Tick extraction on active pixels
    let max_perp = (min_dim * 0.04).max(15.0).min(60.0);
    let max_t_extent = ((w.max(h) as usize) * 3).max(8192);
    
    let (mut x_ticks, _x_body, x_t_start, x_t_end) = x_line
        .as_ref()
        .map(|l| extract_ticks(&active_pixels, l, max_perp, max_t_extent))
        .unwrap_or((Vec::new(), 5.0, 0, 0));
    let (mut y_ticks, _y_body, y_t_start, y_t_end) = y_line
        .as_ref()
        .map(|l| extract_ticks(&active_pixels, l, max_perp, max_t_extent))
        .unwrap_or((Vec::new(), 5.0, 0, 0));

    // Sort ticks consistently: left→right for X, top→bottom for Y
    x_ticks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    y_ticks.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // Step 5: Pixel highlighting (strictly connected pixels to exclude disjoint text)
    let x_axis_pixels = x_line
        .as_ref()
        .map(|l| collect_connected_axis_pixels(&active_pixels, l, max_perp, max_t_extent, x_t_start, x_t_end))
        .unwrap_or_default();
    let y_axis_pixels = y_line
        .as_ref()
        .map(|l| collect_connected_axis_pixels(&active_pixels, l, max_perp, max_t_extent, y_t_start, y_t_end))
        .unwrap_or_default();

    // Step 6: Endpoints from outermost ticks or pixel extent
    let x_axis = x_line
        .as_ref()
        .map(|l| determine_endpoints(l, &x_ticks, &x_axis_pixels, true));
    let y_axis = y_line
        .as_ref()
        .map(|l| determine_endpoints(l, &y_ticks, &y_axis_pixels, false));

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
//  Step 1: Foreground extraction
// ────────────────────────────────────────────────────────────────

fn build_foreground_image(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
) -> GrayImage {
    let n = (w as usize) * (h as usize);
    let mut raw = vec![0u8; n];
    for i in 0..n {
        if mask[i] {
            let off = i * 4;
            if off + 2 < rgba.len() {
                let px = [rgba[off], rgba[off + 1], rgba[off + 2]];
                if !is_bg_color(px, bg_color) {
                    raw[i] = 255;
                }
            }
        }
    }
    GrayImage::from_raw(w, h, raw).expect("buffer size mismatch")
}

// ────────────────────────────────────────────────────────────────
//  Step 2: Directional morphological opening
//  Opening = erosion + dilation with a rectangular kernel (kw × kh).
//  - (K, 1) preserves horizontal structures ≥ K pixels wide
//  - (1, K) preserves vertical structures ≥ K pixels tall
//  Implemented via separable 1D prefix-sum passes for O(n) cost.
// ────────────────────────────────────────────────────────────────

fn directional_open(img: &GrayImage, kw: u32, kh: u32) -> GrayImage {
    let mut r = img.clone();
    if kw > 1 {
        r = erode_rows(&r, kw);
        r = dilate_rows(&r, kw);
    }
    if kh > 1 {
        r = erode_cols(&r, kh);
        r = dilate_cols(&r, kh);
    }
    r
}

/// Row-wise erosion: pixel survives only if all `k` consecutive pixels are foreground.
fn erode_rows(img: &GrayImage, k: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let half = (k / 2) as usize;
    let wu = w as usize;
    let mut dst = vec![0u8; src.len()];

    let mut pfx = vec![0u32; wu + 1];
    for y in 0..h as usize {
        let row = y * wu;
        // Prefix count of background pixels
        pfx[0] = 0;
        for x in 0..wu {
            pfx[x + 1] = pfx[x] + u32::from(src[row + x] == 0);
        }
        for x in 0..wu {
            let lo = x.saturating_sub(half);
            let hi = (x + half).min(wu - 1);
            // No bg pixels in window → survives erosion
            if pfx[hi + 1] == pfx[lo] {
                dst[row + x] = 255;
            }
        }
    }
    GrayImage::from_raw(w, h, dst).unwrap()
}

/// Row-wise dilation: pixel is set if any of `k` consecutive pixels are foreground.
fn dilate_rows(img: &GrayImage, k: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let half = (k / 2) as usize;
    let wu = w as usize;
    let mut dst = vec![0u8; src.len()];

    let mut pfx = vec![0u32; wu + 1];
    for y in 0..h as usize {
        let row = y * wu;
        // Prefix count of foreground pixels
        pfx[0] = 0;
        for x in 0..wu {
            pfx[x + 1] = pfx[x] + u32::from(src[row + x] > 0);
        }
        for x in 0..wu {
            let lo = x.saturating_sub(half);
            let hi = (x + half).min(wu - 1);
            if pfx[hi + 1] > pfx[lo] {
                dst[row + x] = 255;
            }
        }
    }
    GrayImage::from_raw(w, h, dst).unwrap()
}

/// Col-wise erosion: pixel survives only if all `k` consecutive pixels are foreground.
fn erode_cols(img: &GrayImage, k: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let half = (k / 2) as usize;
    let wu = w as usize;
    let hu = h as usize;
    let mut dst = vec![0u8; src.len()];

    let mut pfx = vec![0u32; hu + 1];
    for x in 0..wu {
        pfx[0] = 0;
        for y in 0..hu {
            pfx[y + 1] = pfx[y] + u32::from(src[y * wu + x] == 0);
        }
        for y in 0..hu {
            let lo = y.saturating_sub(half);
            let hi = (y + half).min(hu - 1);
            if pfx[hi + 1] == pfx[lo] {
                dst[y * wu + x] = 255;
            }
        }
    }
    GrayImage::from_raw(w, h, dst).unwrap()
}

/// Col-wise dilation: pixel is set if any of `k` consecutive pixels are foreground.
fn dilate_cols(img: &GrayImage, k: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let half = (k / 2) as usize;
    let wu = w as usize;
    let hu = h as usize;
    let mut dst = vec![0u8; src.len()];

    let mut pfx = vec![0u32; hu + 1];
    for x in 0..wu {
        pfx[0] = 0;
        for y in 0..hu {
            pfx[y + 1] = pfx[y] + u32::from(src[y * wu + x] > 0);
        }
        for y in 0..hu {
            let lo = y.saturating_sub(half);
            let hi = (y + half).min(hu - 1);
            if pfx[hi + 1] > pfx[lo] {
                dst[y * wu + x] = 255;
            }
        }
    }
    GrayImage::from_raw(w, h, dst).unwrap()
}

// ────────────────────────────────────────────────────────────────
//  Step 3: Hough line detection with angle filtering
// ────────────────────────────────────────────────────────────────

fn downscale_2x(img: &GrayImage) -> (GrayImage, f32) {
    let (w, h) = img.dimensions();
    if w < 2 || h < 2 {
        return (img.clone(), 1.0);
    }
    let nw = w / 2;
    let nh = h / 2;
    let src = img.as_raw();
    let mut dst = vec![0u8; (nw * nh) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let src_idx = (y * 2 * w + x * 2) as usize;
            dst[(y * nw + x) as usize] = src[src_idx];
        }
    }
    (GrayImage::from_raw(nw, nh, dst).unwrap(), 2.0)
}

fn detect_best_line(opened: &GrayImage, active_pixels: &[(f32, f32)], is_horizontal: bool) -> Option<AxisLine> {
    let (opened_small, scale) = downscale_2x(opened);
    let (w, h) = opened_small.dimensions();
    let min_dim = w.min(h);

    let vote_threshold = (min_dim as f32 * 0.08).max(15.0) as u32;
    let options = LineDetectionOptions {
        vote_threshold,
        suppression_radius: 5,
    };
    let lines = detect_lines(&opened_small, options);

    // Angle filter:
    //   Horizontal (X-axis, normal ≈ 90°): angle ∈ [85, 95]
    //   Vertical   (Y-axis, normal ≈ 0°):  angle ∈ [0, 5] ∪ [175, 179]
    let candidates: Vec<AxisLine> = lines
        .iter()
        .filter(|l| {
            let a = l.angle_in_degrees;
            if is_horizontal {
                (85..=95).contains(&a)
            } else {
                a <= 5 || a >= 175
            }
        })
        .map(|l| {
            let mut line = AxisLine::from_polar(l);
            line.r *= scale;
            line
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Pick the line with the most foreground pixel support
    candidates
        .iter()
        .enumerate()
        .max_by_key(|(_, line)| count_support(active_pixels, line, 3.0))
        .map(|(i, _)| candidates[i])
}

fn count_support(active_pixels: &[(f32, f32)], line: &AxisLine, tolerance: f32) -> u32 {
    let mut count = 0u32;
    for &(x, y) in active_pixels {
        if line.perp_dist(x, y).abs() <= tolerance {
            count += 1;
        }
    }
    count
}

// ────────────────────────────────────────────────────────────────
//  Step 4: Tick extraction via 1D perpendicular-extent profiling
// ────────────────────────────────────────────────────────────────

/// Returns (tick positions, estimated body half-thickness).
fn extract_ticks(
    active_pixels: &[(f32, f32)],
    line: &AxisLine,
    max_perp: f32,
    max_t_extent: usize,
) -> (Vec<(f32, f32)>, f32, i32, i32) {
    let offset = (max_t_extent / 2) as i32;
    let mut grid_pos = vec![0u64; max_t_extent];
    let mut grid_neg = vec![0u64; max_t_extent];
    let mut min_t_found = i32::MAX;
    let mut max_t_found = i32::MIN;

    // Build 2D occupancy grid limit to max_perp <= 60
    for &(x, y) in active_pixels {
        let d = line.perp_dist(x, y);
        let abs_d = d.abs();
        if abs_d <= max_perp {
            let t = line.project(x, y).round() as i32;
            let tu = (t + offset) as usize;
            
            // Boundary safety check
            if tu < max_t_extent {
                let d_bin = abs_d.round() as usize;
                if d_bin < 64 {
                    if d >= 0.0 {
                        grid_pos[tu] |= 1 << d_bin;
                    } else {
                        grid_neg[tu] |= 1 << d_bin;
                    }
                }
                min_t_found = min_t_found.min(t);
                max_t_found = max_t_found.max(t);
            }
        }
    }

    if min_t_found > max_t_found {
        return (Vec::new(), 1.0, 0, 0);
    }

    let mut profile = vec![-1.0f32; max_t_extent];

    for t in min_t_found..=max_t_found {
        let tu = (t + offset) as usize;
        let ext_pos = connected_extent(grid_pos[tu]);
        let ext_neg = connected_extent(grid_neg[tu]);
        let max_ext = ext_pos.max(ext_neg);
        
        profile[tu] = max_ext;
    }

    // Segment t bounds to ignore disconnected ends (like disjoint text)
    let max_gap = 10;
    let mut best_start = min_t_found;
    let mut best_end = max_t_found;
    let mut current_start = min_t_found;
    let mut last_valid = min_t_found;
    let mut in_segment = false;
    let mut max_len = 0;

    for t in min_t_found..=max_t_found {
        let tu = (t + offset) as usize;
        let ext = profile[tu];
        if ext >= 0.0 {
            if !in_segment {
                current_start = t;
                in_segment = true;
            } else if t - last_valid > max_gap {
                // finish previous segment
                let len = last_valid - current_start;
                if len > max_len {
                    max_len = len;
                    best_start = current_start;
                    best_end = last_valid;
                }
                current_start = t;
            }
            last_valid = t;
        }
    }
    if in_segment {
        let len = last_valid - current_start;
        if len >= max_len {
            best_start = current_start;
            best_end = last_valid;
        }
    }

    let mut sorted_t: Vec<i32> = Vec::new();
    let mut exts: Vec<f32> = Vec::new();

    for t in best_start..=best_end {
        let tu = (t + offset) as usize;
        let ext = profile[tu];
        if ext >= 0.0 {
            sorted_t.push(t);
            exts.push(ext);
        }
    }

    if exts.is_empty() {
        return (Vec::new(), 1.0, best_start, best_end);
    }

    // Median perpendicular extent ≈ axis body half-thickness
    let mut sorted_exts = exts.clone();
    sorted_exts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_ext = sorted_exts[sorted_exts.len() / 2].max(1.0);

    let tick_min_ext = (median_ext * 2.0).max(2.0);
    let span = (*sorted_t.last().unwrap() - *sorted_t.first().unwrap() + 1) as f32;
    let tick_max_w = (span * 0.05).max(3.0) as i32;

    // Scan for bumps (consecutive positions with extent ≥ tick threshold)
    let mut ticks: Vec<(f32, f32)> = Vec::new();
    let mut run: Vec<i32> = Vec::new();

    for &t in &sorted_t {
        let tu = (t + offset) as usize;
        if profile[tu] >= tick_min_ext {
            if !run.is_empty() && t > *run.last().unwrap() + 1 {
                flush_bump(&run, &mut ticks, tick_max_w, line);
                run.clear();
            }
            run.push(t);
        } else if !run.is_empty() {
            flush_bump(&run, &mut ticks, tick_max_w, line);
            run.clear();
        }
    }
    if !run.is_empty() {
        flush_bump(&run, &mut ticks, tick_max_w, line);
    }

    // Periodicity filtering
    if ticks.len() >= 3 {
        filter_by_periodicity(&mut ticks, line);
    }

    (ticks, median_ext, best_start, best_end)
}

fn flush_bump(run: &[i32], ticks: &mut Vec<(f32, f32)>, max_w: i32, line: &AxisLine) {
    if run.is_empty() {
        return;
    }
    let w = run.last().unwrap() - run.first().unwrap() + 1;
    if w <= max_w {
        let center = (*run.first().unwrap() + *run.last().unwrap()) as f32 / 2.0;
        ticks.push(line.point_at(center));
    }
}

/// Reject outlier ticks whose inter-spacing doesn't match the dominant periodicity.
fn filter_by_periodicity(ticks: &mut Vec<(f32, f32)>, line: &AxisLine) {
    if ticks.len() < 3 {
        return;
    }

    let t_vals: Vec<f32> = ticks.iter().map(|&(x, y)| line.project(x, y)).collect();
    let gaps: Vec<f32> = t_vals.windows(2).map(|w| (w[1] - w[0]).abs()).collect();

    let mut sorted_gaps = gaps.clone();
    sorted_gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_gap = sorted_gaps[sorted_gaps.len() / 2];

    if median_gap < 3.0 {
        return;
    }

    // A gap is "good" if it approximates an integer multiple of the median spacing
    let is_good = |g: f32| -> bool {
        let ratio = g / median_gap;
        let nearest = ratio.round();
        nearest >= 0.5 && (ratio - nearest).abs() < 0.35
    };

    // Keep ticks that border at least one good gap
    let mut keep = vec![false; ticks.len()];
    for (i, &g) in gaps.iter().enumerate() {
        if is_good(g) {
            keep[i] = true;
            keep[i + 1] = true;
        }
    }

    *ticks = ticks
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, &t)| t)
        .collect();
}

// ────────────────────────────────────────────────────────────────
//  Step 5: Pixel collection for UI highlighting
// ────────────────────────────────────────────────────────────────

fn connected_extent(bits: u64) -> f32 {
    let mut max_d = -1.0;
    let mut zeros = 0;
    let mut started = false;
    for d_bin in 0..64 {
        if (bits & (1 << d_bin)) != 0 {
            started = true;
            max_d = d_bin as f32;
            zeros = 0;
        } else if started {
            zeros += 1;
            if zeros >= 3 {
                break;
            }
        } else {
            if d_bin >= 3 {
                break;
            }
        }
    }
    max_d
}

fn collect_connected_axis_pixels(
    active_pixels: &[(f32, f32)],
    line: &AxisLine,
    max_perp: f32,
    max_t_extent: usize,
    t_start: i32,
    t_end: i32,
) -> Vec<(u32, u32)> {
    let offset = (max_t_extent / 2) as i32;
    let mut grid_pos = vec![0u64; max_t_extent];
    let mut grid_neg = vec![0u64; max_t_extent];

    for &(x, y) in active_pixels {
        let t = line.project(x, y).round() as i32;
        if t < t_start || t > t_end {
            continue;
        }
        let d = line.perp_dist(x, y);
        let abs_d = d.abs();
        if abs_d <= max_perp {
            let t = line.project(x, y).round() as i32;
            let tu = (t + offset) as usize;
            if tu < max_t_extent {
                let d_bin = abs_d.round() as usize;
                if d_bin < 64 {
                    if d >= 0.0 {
                        grid_pos[tu] |= 1 << d_bin;
                    } else {
                        grid_neg[tu] |= 1 << d_bin;
                    }
                }
            }
        }
    }

    let mut ext_pos_map = vec![-1.0f32; max_t_extent];
    let mut ext_neg_map = vec![-1.0f32; max_t_extent];

    for tu in 0..max_t_extent {
        if grid_pos[tu] != 0 {
            ext_pos_map[tu] = connected_extent(grid_pos[tu]);
        }
        if grid_neg[tu] != 0 {
            ext_neg_map[tu] = connected_extent(grid_neg[tu]);
        }
    }

    let mut pixels = Vec::new();
    for &(x, y) in active_pixels {
        let t = line.project(x, y).round() as i32;
        if t < t_start || t > t_end {
            continue;
        }
        let d = line.perp_dist(x, y);
        let abs_d = d.abs();
        if abs_d <= max_perp {
            let t = line.project(x, y).round() as i32;
            let tu = (t + offset) as usize;
            if tu < max_t_extent {
                let limit = if d >= 0.0 { ext_pos_map[tu] } else { ext_neg_map[tu] };
                if limit >= 0.0 && abs_d <= limit + 1.0 {
                    pixels.push((x as u32, y as u32));
                }
            }
        }
    }
    pixels
}

// ────────────────────────────────────────────────────────────────
//  Step 6: Determine axis endpoints
// ────────────────────────────────────────────────────────────────

fn determine_endpoints(
    line: &AxisLine,
    ticks: &[(f32, f32)],
    pixels: &[(u32, u32)],
    is_horizontal: bool,
) -> ((f32, f32), (f32, f32)) {
    let (a, b) = if !ticks.is_empty() {
        (*ticks.first().unwrap(), *ticks.last().unwrap())
    } else if !pixels.is_empty() {
        let mut min_t = f32::MAX;
        let mut max_t = f32::MIN;
        for &(x, y) in pixels {
            let t = line.project(x as f32, y as f32);
            min_t = min_t.min(t);
            max_t = max_t.max(t);
        }
        (line.point_at(min_t), line.point_at(max_t))
    } else {
        let p = line.point_at(0.0);
        (p, p)
    };

    // Consistent ordering: left→right for X-axis, top→bottom for Y-axis
    if is_horizontal {
        if a.0 <= b.0 { (a, b) } else { (b, a) }
    } else {
        if a.1 <= b.1 { (a, b) } else { (b, a) }
    }
}

