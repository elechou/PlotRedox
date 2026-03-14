mod line;
mod preprocess;
#[cfg(test)]
mod tests;
mod ticks;

use crate::state::AxisDetectionResult;

use self::line::{
    detect_best_line, determine_endpoints, intersect_lines, trim_axis_to_intersection_side,
    AxisSideSelection,
};
use self::preprocess::{build_foreground_image, collect_active_pixels, directional_open};
use self::ticks::{collect_connected_axis_pixels, extract_ticks};

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

    let fg = build_foreground_image(rgba, mask, w, h, bg_color);
    let active_pixels = collect_active_pixels(&fg);

    if active_pixels.len() < 20 {
        return empty;
    }

    let min_dim = w.min(h) as f32;
    let kernel_len = (min_dim * 0.04).max(20.0) as u32;
    let h_open = directional_open(&fg, kernel_len, 1);
    let v_open = directional_open(&fg, 1, kernel_len);

    let x_line = detect_best_line(&h_open, true);
    let y_line = detect_best_line(&v_open, false);
    if x_line.is_none() && y_line.is_none() {
        return empty;
    }

    let max_perp = (min_dim * 0.04).max(15.0).min(60.0);
    let max_t_extent = ((w.max(h) as usize) * 3).max(8192);

    let (mut x_ticks, x_body, x_t_start, x_t_end) = x_line
        .as_ref()
        .map(|l| extract_ticks(&active_pixels, l, max_perp, max_t_extent))
        .unwrap_or((Vec::new(), 5.0, 0, 0));
    let (mut y_ticks, y_body, y_t_start, y_t_end) = y_line
        .as_ref()
        .map(|l| extract_ticks(&active_pixels, l, max_perp, max_t_extent))
        .unwrap_or((Vec::new(), 5.0, 0, 0));

    x_ticks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    y_ticks.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut x_axis_pixels = x_line
        .as_ref()
        .map(|l| collect_connected_axis_pixels(&active_pixels, l, x_body, x_t_start, x_t_end))
        .unwrap_or_default();
    let mut y_axis_pixels = y_line
        .as_ref()
        .map(|l| collect_connected_axis_pixels(&active_pixels, l, y_body, y_t_start, y_t_end))
        .unwrap_or_default();

    let mut intersection = None;
    let mut x_side_selection = AxisSideSelection::Unknown;
    let mut y_side_selection = AxisSideSelection::Unknown;

    if let (Some(xl), Some(yl)) = (x_line.as_ref(), y_line.as_ref()) {
        if let Some(origin) = intersect_lines(xl, yl) {
            intersection = Some(origin);
            x_side_selection = trim_axis_to_intersection_side(
                xl,
                &mut x_ticks,
                &mut x_axis_pixels,
                origin,
                x_body,
            );
            y_side_selection = trim_axis_to_intersection_side(
                yl,
                &mut y_ticks,
                &mut y_axis_pixels,
                origin,
                y_body,
            );
        }
    }

    x_ticks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    y_ticks.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let x_axis = x_line.as_ref().map(|l| {
        determine_endpoints(
            l,
            &x_ticks,
            &x_axis_pixels,
            true,
            intersection,
            x_side_selection,
        )
    });
    let y_axis = y_line.as_ref().map(|l| {
        determine_endpoints(
            l,
            &y_ticks,
            &y_axis_pixels,
            false,
            intersection,
            y_side_selection,
        )
    });

    AxisDetectionResult {
        x_axis,
        y_axis,
        x_axis_pixels,
        y_axis_pixels,
        x_ticks,
        y_ticks,
    }
}
