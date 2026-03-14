use std::collections::{HashSet, VecDeque};

pub(crate) fn split_into_connected_components(
    pixels: &[(u32, u32)],
    neighbor_radius: u32,
) -> Vec<Vec<(u32, u32)>> {
    if pixels.is_empty() {
        return Vec::new();
    }

    let pixel_set: HashSet<(u32, u32)> = pixels.iter().copied().collect();
    let mut visited: HashSet<(u32, u32)> = HashSet::with_capacity(pixel_set.len());
    let mut components = Vec::new();
    let radius = neighbor_radius as i32;

    for &start in pixels {
        if visited.contains(&start) {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited.insert(start);

        while let Some((x, y)) = queue.pop_front() {
            component.push((x, y));

            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 {
                        continue;
                    }

                    let next = (nx as u32, ny as u32);
                    if !pixel_set.contains(&next) || visited.contains(&next) {
                        continue;
                    }

                    visited.insert(next);
                    queue.push_back(next);
                }
            }
        }

        components.push(component);
    }

    components.sort_by(|a, b| b.len().cmp(&a.len()));
    components
}
