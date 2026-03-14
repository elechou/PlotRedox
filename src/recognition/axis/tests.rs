use super::line::{
    determine_endpoints, intersect_lines, score_line_support, trim_axis_to_intersection_side,
    AxisLine, AxisSideSelection,
};
use super::ticks::{collect_connected_axis_pixels, extract_ticks};
use image::{GrayImage, Luma};

#[test]
fn continuous_axis_scores_above_disconnected_digit_stems() {
    let mut img = GrayImage::new(64, 64);

    for y in 4..60 {
        img.put_pixel(40, y, Luma([255]));
    }

    for y in 6..16 {
        img.put_pixel(18, y, Luma([255]));
    }
    for y in 28..38 {
        img.put_pixel(18, y, Luma([255]));
    }
    for y in 48..58 {
        img.put_pixel(18, y, Luma([255]));
    }

    let axis_line = AxisLine {
        cos_t: 1.0,
        sin_t: 0.0,
        r: 40.0,
    };
    let digit_line = AxisLine {
        cos_t: 1.0,
        sin_t: 0.0,
        r: 18.0,
    };

    assert!(score_line_support(&img, &axis_line, 1.5) > score_line_support(&img, &digit_line, 1.5));
}

#[test]
fn axis_pixel_collection_stays_near_axis_body() {
    let mut active_pixels = Vec::new();

    for y in 0..=20 {
        active_pixels.push((10.0, y as f32));
    }

    for x in 11..=14 {
        active_pixels.push((x as f32, 10.0));
    }

    for x in 18..=20 {
        for y in 9..=11 {
            active_pixels.push((x as f32, y as f32));
        }
    }

    let line = AxisLine {
        cos_t: 1.0,
        sin_t: 0.0,
        r: 10.0,
    };

    let pixels = collect_connected_axis_pixels(&active_pixels, &line, 1.0, 0, 20);

    assert!(pixels.iter().any(|&(x, y)| x == 10 && y == 10));
    assert!(pixels.iter().all(|&(x, _)| x < 18));
}

#[test]
fn trims_x_axis_extension_side_near_intersection() {
    let x_line = AxisLine {
        cos_t: 0.0,
        sin_t: 1.0,
        r: 40.0,
    };
    let y_line = AxisLine {
        cos_t: 1.0,
        sin_t: 0.0,
        r: 30.0,
    };
    let origin = intersect_lines(&x_line, &y_line).unwrap();

    let mut ticks = vec![(18.0, 40.0), (46.0, 40.0), (62.0, 40.0), (78.0, 40.0)];
    let mut pixels = (14..=22)
        .map(|x| (x as u32, 40u32))
        .chain((34..=82).map(|x| (x as u32, 40u32)))
        .collect::<Vec<_>>();

    trim_axis_to_intersection_side(&x_line, &mut ticks, &mut pixels, origin, 1.0);

    assert!(ticks.iter().all(|&(x, _)| x > 30.0));
    assert!(pixels.iter().all(|&(x, _)| x >= 23));
    assert!(ticks.len() >= 3);
    assert!(pixels.iter().any(|&(x, _)| (23..=37).contains(&x)));
}

#[test]
fn keeps_both_sides_when_both_have_real_axis_support() {
    let line = AxisLine {
        cos_t: 0.0,
        sin_t: 1.0,
        r: 40.0,
    };
    let intersection = (30.0, 40.0);

    let mut ticks = vec![(10.0, 40.0), (20.0, 40.0), (45.0, 40.0), (55.0, 40.0)];
    let mut pixels = (8..=24)
        .map(|x| (x as u32, 40u32))
        .chain((36..=60).map(|x| (x as u32, 40u32)))
        .collect::<Vec<_>>();

    trim_axis_to_intersection_side(&line, &mut ticks, &mut pixels, intersection, 1.0);

    assert!(ticks.iter().any(|&(x, _)| x < 30.0));
    assert!(ticks.iter().any(|&(x, _)| x > 30.0));
    assert!(pixels.iter().any(|&(x, _)| x < 30));
    assert!(pixels.iter().any(|&(x, _)| x > 30));
}

#[test]
fn single_sided_axis_snaps_endpoint_back_to_intersection() {
    let line = AxisLine {
        cos_t: 0.0,
        sin_t: 1.0,
        r: 40.0,
    };
    let ticks = vec![(46.0, 40.0), (62.0, 40.0), (78.0, 40.0)];
    let pixels = (30..=82).map(|x| (x as u32, 40u32)).collect::<Vec<_>>();

    let axis = determine_endpoints(
        &line,
        &ticks,
        &pixels,
        true,
        Some((30.0, 40.0)),
        AxisSideSelection::PositiveOnly,
    );

    assert!((axis.0 .0 - 30.0).abs() < 0.01);
    assert!((axis.0 .1 - 40.0).abs() < 0.01);
    assert!(axis.1 .0 > 70.0);
}

#[test]
fn extract_ticks_recovers_endpoint_box_corner() {
    let line = AxisLine {
        cos_t: 1.0,
        sin_t: 0.0,
        r: 10.0,
    };
    let mut active_pixels = Vec::new();

    for y in 0..=100 {
        active_pixels.push((10.0, y as f32));
    }

    for &(y, x_end) in &[(20.0, 14), (40.0, 14), (60.0, 14), (80.0, 14)] {
        for x in 11..=x_end {
            active_pixels.push((x as f32, y));
        }
    }

    for x in 14..=26 {
        active_pixels.push((x as f32, 0.0));
    }

    let (ticks, _, _, _) = extract_ticks(&active_pixels, &line, 20.0, 512);
    let mut y_positions: Vec<f32> = ticks.iter().map(|&(_, y)| y).collect();
    y_positions.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!(y_positions.iter().any(|&y| y <= 2.0));
    assert!(y_positions.iter().any(|&y| (18.0..=22.0).contains(&y)));
    assert!(y_positions.iter().any(|&y| (78.0..=82.0).contains(&y)));
}

#[test]
fn determine_endpoints_orders_y_axis_bottom_to_top() {
    let line = AxisLine {
        cos_t: 1.0,
        sin_t: 0.0,
        r: 10.0,
    };
    let ticks = vec![(10.0, 12.0), (10.0, 88.0)];

    let axis = determine_endpoints(
        &line,
        &ticks,
        &[],
        false,
        None,
        AxisSideSelection::BothSides,
    );

    assert!((axis.0 .1 - 88.0).abs() < 0.01);
    assert!((axis.1 .1 - 12.0).abs() < 0.01);
}
