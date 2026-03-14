use image::GrayImage;
use imageproc::hough::{detect_lines, LineDetectionOptions, PolarLine};

#[derive(Clone, Copy)]
pub(super) struct AxisLine {
    pub(super) cos_t: f32,
    pub(super) sin_t: f32,
    pub(super) r: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AxisSideSelection {
    Unknown,
    NegativeOnly,
    PositiveOnly,
    BothSides,
    CoreOnly,
}

impl AxisSideSelection {
    pub(super) fn is_single_sided(self) -> bool {
        matches!(self, Self::NegativeOnly | Self::PositiveOnly)
    }
}

impl AxisLine {
    pub(super) fn from_polar(pl: &PolarLine) -> Self {
        let theta = (pl.angle_in_degrees as f32).to_radians();
        let (sin_t, cos_t) = theta.sin_cos();
        Self {
            cos_t,
            sin_t,
            r: pl.r,
        }
    }

    #[inline]
    pub(super) fn perp_dist(&self, x: f32, y: f32) -> f32 {
        x * self.cos_t + y * self.sin_t - self.r
    }

    #[inline]
    pub(super) fn project(&self, x: f32, y: f32) -> f32 {
        -x * self.sin_t + y * self.cos_t
    }

    pub(super) fn point_at(&self, t: f32) -> (f32, f32) {
        (
            self.r * self.cos_t - t * self.sin_t,
            self.r * self.sin_t + t * self.cos_t,
        )
    }
}

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

pub(super) fn detect_best_line(opened: &GrayImage, is_horizontal: bool) -> Option<AxisLine> {
    let (opened_small, scale) = downscale_2x(opened);
    let (w, h) = opened_small.dimensions();
    let min_dim = w.min(h);

    let vote_threshold = (min_dim as f32 * 0.08).max(15.0) as u32;
    let options = LineDetectionOptions {
        vote_threshold,
        suppression_radius: 5,
    };
    let lines = detect_lines(&opened_small, options);

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

    candidates
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            score_line_support(opened, a, 2.5).total_cmp(&score_line_support(opened, b, 2.5))
        })
        .map(|(i, _)| candidates[i])
}

pub(super) fn score_line_support(opened: &GrayImage, line: &AxisLine, tolerance: f32) -> f32 {
    let (w, h) = opened.dimensions();
    let src = opened.as_raw();
    let wu = w as usize;

    let mut support_pixels = 0u32;
    let mut t_bins = Vec::new();

    for y in 0..h as usize {
        let row = y * wu;
        for x in 0..wu {
            if src[row + x] == 0 {
                continue;
            }

            let xf = x as f32;
            let yf = y as f32;
            if line.perp_dist(xf, yf).abs() <= tolerance {
                support_pixels += 1;
                t_bins.push(line.project(xf, yf).round() as i32);
            }
        }
    }

    if t_bins.is_empty() {
        return f32::NEG_INFINITY;
    }

    t_bins.sort_unstable();

    let min_t = *t_bins.first().unwrap();
    let max_t = *t_bins.last().unwrap();
    let span = (max_t - min_t + 1).max(1) as f32;

    let mut occupied_bins = 0u32;
    let mut longest_run = 0u32;
    let mut current_run = 0u32;
    let mut last_t = i32::MIN;

    for t in t_bins {
        if t == last_t {
            continue;
        }

        occupied_bins += 1;
        if last_t != i32::MIN && t == last_t + 1 {
            current_run += 1;
        } else {
            current_run = 1;
        }
        longest_run = longest_run.max(current_run);
        last_t = t;
    }

    let continuity = longest_run as f32 / span;
    let fill_ratio = occupied_bins as f32 / span;

    support_pixels as f32
        + occupied_bins as f32 * 4.0
        + longest_run as f32 * 16.0
        + continuity * 200.0
        + fill_ratio * 100.0
}

pub(super) fn intersect_lines(a: &AxisLine, b: &AxisLine) -> Option<(f32, f32)> {
    let det = a.cos_t * b.sin_t - a.sin_t * b.cos_t;
    if det.abs() < 1e-4 {
        return None;
    }

    Some((
        (a.r * b.sin_t - a.sin_t * b.r) / det,
        (a.cos_t * b.r - a.r * b.cos_t) / det,
    ))
}

fn projected_tick_span(line: &AxisLine, ticks: &[(f32, f32)]) -> f32 {
    if ticks.len() < 2 {
        return 0.0;
    }

    let mut min_t = f32::MAX;
    let mut max_t = f32::MIN;
    for &(x, y) in ticks {
        let t = line.project(x, y);
        min_t = min_t.min(t);
        max_t = max_t.max(t);
    }
    (max_t - min_t).max(0.0)
}

fn projected_pixel_span(line: &AxisLine, pixels: &[(u32, u32)]) -> f32 {
    if pixels.len() < 2 {
        return 0.0;
    }

    let mut min_t = f32::MAX;
    let mut max_t = f32::MIN;
    for &(x, y) in pixels {
        let t = line.project(x as f32, y as f32);
        min_t = min_t.min(t);
        max_t = max_t.max(t);
    }
    (max_t - min_t).max(0.0)
}

fn side_score(line: &AxisLine, ticks: &[(f32, f32)], pixels: &[(u32, u32)]) -> f32 {
    let tick_count = ticks.len() as f32;
    let tick_span = projected_tick_span(line, ticks);
    let pixel_span = projected_pixel_span(line, pixels);
    let span = tick_span.max(pixel_span);

    let tick_score = if ticks.len() >= 2 {
        400.0 + tick_count * 120.0 + tick_span * 2.5
    } else {
        tick_count * 40.0
    };

    tick_score + span * 2.0 + pixels.len() as f32 * 0.1
}

pub(super) fn trim_axis_to_intersection_side(
    line: &AxisLine,
    ticks: &mut Vec<(f32, f32)>,
    pixels: &mut Vec<(u32, u32)>,
    intersection: (f32, f32),
    body_half_thickness: f32,
) -> AxisSideSelection {
    let t_origin = line.project(intersection.0, intersection.1);
    let core_zone = (body_half_thickness * 3.0 + 4.0).clamp(4.0, 14.0);

    let mut neg_ticks = Vec::new();
    let mut core_ticks = Vec::new();
    let mut pos_ticks = Vec::new();
    for &tick in ticks.iter() {
        let t = line.project(tick.0, tick.1);
        if t < t_origin - core_zone {
            neg_ticks.push(tick);
        } else if t > t_origin + core_zone {
            pos_ticks.push(tick);
        } else {
            core_ticks.push(tick);
        }
    }

    let mut neg_pixels = Vec::new();
    let mut core_pixels = Vec::new();
    let mut pos_pixels = Vec::new();
    for &pixel in pixels.iter() {
        let t = line.project(pixel.0 as f32, pixel.1 as f32);
        if t < t_origin - core_zone {
            neg_pixels.push(pixel);
        } else if t > t_origin + core_zone {
            pos_pixels.push(pixel);
        } else {
            core_pixels.push(pixel);
        }
    }

    let neg_nonempty = !neg_ticks.is_empty() || !neg_pixels.is_empty();
    let pos_nonempty = !pos_ticks.is_empty() || !pos_pixels.is_empty();
    let core_nonempty = !core_ticks.is_empty() || !core_pixels.is_empty();

    if !neg_nonempty && !pos_nonempty {
        *ticks = core_ticks;
        *pixels = core_pixels;
        return if core_nonempty {
            AxisSideSelection::CoreOnly
        } else {
            AxisSideSelection::Unknown
        };
    }

    if !neg_nonempty {
        *ticks = core_ticks;
        ticks.extend(pos_ticks);
        *pixels = core_pixels;
        pixels.extend(pos_pixels);
        return AxisSideSelection::PositiveOnly;
    }
    if !pos_nonempty {
        *ticks = neg_ticks;
        ticks.extend(core_ticks);
        *pixels = neg_pixels;
        pixels.extend(core_pixels);
        return AxisSideSelection::NegativeOnly;
    }

    let neg_score = side_score(line, &neg_ticks, &neg_pixels);
    let pos_score = side_score(line, &pos_ticks, &pos_pixels);

    let keep_positive = if pos_ticks.len() >= 2 && neg_ticks.len() < 2 {
        Some(true)
    } else if neg_ticks.len() >= 2 && pos_ticks.len() < 2 {
        Some(false)
    } else if pos_score >= neg_score * 1.35 {
        Some(true)
    } else if neg_score >= pos_score * 1.35 {
        Some(false)
    } else {
        None
    };

    match keep_positive {
        Some(true) => {
            *ticks = core_ticks;
            ticks.extend(pos_ticks);
            *pixels = core_pixels;
            pixels.extend(pos_pixels);
            AxisSideSelection::PositiveOnly
        }
        Some(false) => {
            *ticks = neg_ticks;
            ticks.extend(core_ticks);
            *pixels = neg_pixels;
            pixels.extend(core_pixels);
            AxisSideSelection::NegativeOnly
        }
        None => {
            *ticks = neg_ticks;
            ticks.extend(core_ticks);
            ticks.extend(pos_ticks);
            *pixels = neg_pixels;
            pixels.extend(core_pixels);
            pixels.extend(pos_pixels);
            AxisSideSelection::BothSides
        }
    }
}

pub(super) fn determine_endpoints(
    line: &AxisLine,
    ticks: &[(f32, f32)],
    pixels: &[(u32, u32)],
    is_horizontal: bool,
    intersection: Option<(f32, f32)>,
    side_selection: AxisSideSelection,
) -> ((f32, f32), (f32, f32)) {
    let (mut a, mut b) = if !ticks.is_empty() {
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

    if let Some(origin) = intersection {
        if side_selection.is_single_sided() {
            let t_origin = line.project(origin.0, origin.1);
            let dist_a = (line.project(a.0, a.1) - t_origin).abs();
            let dist_b = (line.project(b.0, b.1) - t_origin).abs();
            if dist_a <= dist_b {
                a = origin;
            } else {
                b = origin;
            }
        } else if side_selection == AxisSideSelection::CoreOnly {
            a = origin;
            b = origin;
        }
    }

    if is_horizontal {
        if a.0 <= b.0 {
            (a, b)
        } else {
            (b, a)
        }
    } else if a.1 >= b.1 {
        (a, b)
    } else {
        (b, a)
    }
}
