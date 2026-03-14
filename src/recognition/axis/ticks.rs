use super::line::AxisLine;

pub(super) fn extract_ticks(
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

    for &(x, y) in active_pixels {
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
        profile[tu] = ext_pos.max(ext_neg);
    }

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

    let mut sorted_t = Vec::new();
    let mut exts = Vec::new();

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

    let mut sorted_exts = exts.clone();
    sorted_exts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let body_ext = sorted_exts[sorted_exts.len() / 4].max(1.0);

    let tick_min_ext = (body_ext * 2.0).max(2.0);
    let span = (*sorted_t.last().unwrap() - *sorted_t.first().unwrap() + 1) as f32;
    let tick_max_w = (span * 0.05).max(3.0) as i32;

    let mut ticks = Vec::new();
    let mut run = Vec::new();

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

    if ticks.len() >= 3 {
        filter_by_periodicity(&mut ticks, line);
    }

    recover_endpoint_ticks(
        &grid_pos, &grid_neg, offset, best_start, best_end, body_ext, tick_max_w, line, &mut ticks,
    );

    (ticks, body_ext, best_start, best_end)
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

    let is_good = |g: f32| -> bool {
        let ratio = g / median_gap;
        let nearest = ratio.round();
        nearest >= 0.5 && (ratio - nearest).abs() < 0.35
    };

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

fn recover_endpoint_ticks(
    grid_pos: &[u64],
    grid_neg: &[u64],
    offset: i32,
    best_start: i32,
    best_end: i32,
    body_ext: f32,
    tick_max_w: i32,
    line: &AxisLine,
    ticks: &mut Vec<(f32, f32)>,
) {
    let edge_margin = (tick_max_w * 2)
        .max((body_ext * 4.0).ceil() as i32 + 2)
        .clamp(4, 24);
    let endpoint_max_w = (tick_max_w * 2)
        .max((body_ext * 5.0).ceil() as i32 + 2)
        .clamp(4, 24);
    let endpoint_min_ext = (body_ext * 1.35).max(2.0);
    let max_gap = ((body_ext.ceil() as usize) + 4).clamp(4, 8);
    let dedupe_gap = (body_ext + 2.0).max(3.0);

    if let Some(center_t) = find_edge_tick_candidate(
        grid_pos,
        grid_neg,
        offset,
        best_start,
        best_end,
        true,
        edge_margin,
        endpoint_max_w,
        endpoint_min_ext,
        max_gap,
    ) {
        push_tick_if_distinct(ticks, line, center_t, dedupe_gap);
    }

    if let Some(center_t) = find_edge_tick_candidate(
        grid_pos,
        grid_neg,
        offset,
        best_start,
        best_end,
        false,
        edge_margin,
        endpoint_max_w,
        endpoint_min_ext,
        max_gap,
    ) {
        push_tick_if_distinct(ticks, line, center_t, dedupe_gap);
    }
}

fn find_edge_tick_candidate(
    grid_pos: &[u64],
    grid_neg: &[u64],
    offset: i32,
    best_start: i32,
    best_end: i32,
    from_start: bool,
    edge_margin: i32,
    endpoint_max_w: i32,
    endpoint_min_ext: f32,
    max_gap: usize,
) -> Option<f32> {
    if best_start > best_end {
        return None;
    }

    let search_start = if from_start {
        best_start
    } else {
        (best_end - edge_margin).max(best_start)
    };
    let search_end = if from_start {
        (best_start + edge_margin).min(best_end)
    } else {
        best_end
    };
    let edge_t = if from_start { best_start } else { best_end };

    let mut best_run: Option<(i32, i32, f32)> = None;
    let mut run_start = 0i32;
    let mut run_end = 0i32;
    let mut run_peak = 0.0f32;
    let mut in_run = false;

    for t in search_start..=search_end {
        let tu = (t + offset) as usize;
        let ext = connected_extent_with_gap(grid_pos[tu], max_gap)
            .max(connected_extent_with_gap(grid_neg[tu], max_gap));

        if ext >= endpoint_min_ext {
            if !in_run {
                run_start = t;
                run_peak = ext;
                in_run = true;
            }
            run_end = t;
            run_peak = run_peak.max(ext);
        } else if in_run {
            maybe_store_edge_run(
                &mut best_run,
                edge_t,
                run_start,
                run_end,
                run_peak,
                endpoint_max_w,
            );
            in_run = false;
        }
    }

    if in_run {
        maybe_store_edge_run(
            &mut best_run,
            edge_t,
            run_start,
            run_end,
            run_peak,
            endpoint_max_w,
        );
    }

    best_run.map(|(start, end, _)| (start + end) as f32 / 2.0)
}

fn maybe_store_edge_run(
    best_run: &mut Option<(i32, i32, f32)>,
    edge_t: i32,
    run_start: i32,
    run_end: i32,
    run_peak: f32,
    endpoint_max_w: i32,
) {
    let width = run_end - run_start + 1;
    let edge_distance = (run_start - edge_t).abs().min((run_end - edge_t).abs());
    if width > endpoint_max_w || edge_distance > 3 {
        return;
    }

    match best_run {
        Some((best_start, best_end, best_peak)) => {
            let best_edge_distance = (*best_start - edge_t).abs().min((*best_end - edge_t).abs());
            if run_peak > *best_peak
                || (run_peak == *best_peak && edge_distance < best_edge_distance)
            {
                *best_run = Some((run_start, run_end, run_peak));
            }
        }
        None => *best_run = Some((run_start, run_end, run_peak)),
    }
}

fn push_tick_if_distinct(
    ticks: &mut Vec<(f32, f32)>,
    line: &AxisLine,
    center_t: f32,
    dedupe_gap: f32,
) {
    if ticks
        .iter()
        .any(|&(x, y)| (line.project(x, y) - center_t).abs() <= dedupe_gap)
    {
        return;
    }
    ticks.push(line.point_at(center_t));
}

fn connected_extent_with_gap(bits: u64, max_gap: usize) -> f32 {
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
            if zeros >= max_gap {
                break;
            }
        } else if d_bin + 1 >= max_gap {
            break;
        }
    }
    max_d
}

fn connected_extent(bits: u64) -> f32 {
    connected_extent_with_gap(bits, 3)
}

pub(super) fn collect_connected_axis_pixels(
    active_pixels: &[(f32, f32)],
    line: &AxisLine,
    body_half_thickness: f32,
    t_start: i32,
    t_end: i32,
) -> Vec<(u32, u32)> {
    let body_band = (body_half_thickness + 1.5).clamp(1.5, 4.0);

    active_pixels
        .iter()
        .filter_map(|&(x, y)| {
            let t = line.project(x, y).round() as i32;
            if t < t_start || t > t_end {
                return None;
            }

            if line.perp_dist(x, y).abs() <= body_band {
                Some((x as u32, y as u32))
            } else {
                None
            }
        })
        .collect()
}
