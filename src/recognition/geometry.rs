use std::cmp::Ordering;

#[derive(Clone, Copy)]
pub(crate) enum DominantAxis {
    Horizontal,
    Vertical,
}

pub(crate) fn bounding_box(points: &[(u32, u32)]) -> Option<(u32, u32, u32, u32)> {
    let mut iter = points.iter().copied();
    let first = iter.next()?;

    let (mut min_x, mut min_y) = first;
    let (mut max_x, mut max_y) = first;

    for (x, y) in iter {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    Some((min_x, min_y, max_x, max_y))
}

pub(crate) fn point_distance_sq(a: (u32, u32), b: (u32, u32)) -> u64 {
    let dx = a.0.abs_diff(b.0) as u64;
    let dy = a.1.abs_diff(b.1) as u64;
    dx * dx + dy * dy
}

pub(crate) fn dominant_axis(points: &[(u32, u32)]) -> DominantAxis {
    let Some((min_x, min_y, max_x, max_y)) = bounding_box(points) else {
        return DominantAxis::Horizontal;
    };

    if max_x - min_x >= max_y - min_y {
        DominantAxis::Horizontal
    } else {
        DominantAxis::Vertical
    }
}

pub(crate) fn compare_points_along_axis(
    a: (u32, u32),
    b: (u32, u32),
    axis: DominantAxis,
) -> Ordering {
    match axis {
        DominantAxis::Horizontal => a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)),
        DominantAxis::Vertical => a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)),
    }
}
