use std::collections::BTreeMap;

use super::super::geometry::{
    compare_points_along_axis, dominant_axis, point_distance_sq, DominantAxis,
};
use super::super::spatial::split_into_connected_components;

// ----------------------------------------------------------------
//  Arc-Length Point Sampling (Multi-Segment)
// ----------------------------------------------------------------

/// Sample N points along a pixel cluster using arc-length parameterization.
/// Handles disconnected curve fragments by sampling each connected chain independently.
pub fn sample_points_from_cluster(pixels: &[(u32, u32)], n: usize, w: u32) -> Vec<(f32, f32)> {
    sample_points_arc_length(pixels, n, w)
}

fn sample_points_arc_length(pixels: &[(u32, u32)], n: usize, _w: u32) -> Vec<(f32, f32)> {
    if pixels.is_empty() || n == 0 {
        return Vec::new();
    }

    let chains = build_pixel_chains(pixels);
    if chains.is_empty() {
        return Vec::new();
    }

    let mut chain_lengths = Vec::with_capacity(chains.len());
    let mut chain_arc_data = Vec::with_capacity(chains.len());

    for chain in &chains {
        let mut arcs = vec![0.0];
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
        return chains
            .iter()
            .flat_map(|c| c.iter().map(|&(x, y)| (x as f32, y as f32)))
            .take(n)
            .collect();
    }

    let total_points_available: usize = chains.iter().map(|c| c.len()).sum();
    if total_points_available <= n {
        return chains
            .iter()
            .flat_map(|c| c.iter().map(|&(x, y)| (x as f32, y as f32)))
            .collect();
    }

    let mut points_per_chain = Vec::with_capacity(chains.len());
    let mut allocated = 0usize;

    for (i, &len) in chain_lengths.iter().enumerate() {
        let share = if grand_total > 0.0 {
            (len / grand_total * n as f32).round() as usize
        } else {
            0
        };
        let share = if chains[i].len() >= 2 {
            share.max(1)
        } else {
            share
        };
        points_per_chain.push(share);
        allocated += share;
    }

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

            let seg = match arcs
                .binary_search_by(|v| v.partial_cmp(&target).unwrap_or(std::cmp::Ordering::Equal))
            {
                Ok(idx) => idx,
                Err(idx) => idx.saturating_sub(1),
            };
            let seg = seg.min(chain.len() - 1);

            if seg + 1 < chain.len() {
                let seg_start = arcs[seg];
                let seg_end = arcs[seg + 1];
                let seg_len = seg_end - seg_start;
                let t = if seg_len > 0.0 {
                    (target - seg_start) / seg_len
                } else {
                    0.0
                };
                let x = chain[seg].0 as f32 + t * (chain[seg + 1].0 as f32 - chain[seg].0 as f32);
                let y = chain[seg].1 as f32 + t * (chain[seg + 1].1 as f32 - chain[seg].1 as f32);
                sampled.push((x, y));
            } else {
                sampled.push((chain[seg].0 as f32, chain[seg].1 as f32));
            }
        }
    }

    sampled
}

/// Build ordered chains from connected curve fragments.
/// Each connected component becomes one chain, which avoids stitching
/// together unrelated points that merely share a color.
fn build_pixel_chains(pixels: &[(u32, u32)]) -> Vec<Vec<(u32, u32)>> {
    if pixels.is_empty() {
        return Vec::new();
    }

    let mut chains = Vec::new();

    for component in split_into_connected_components(pixels, 1) {
        let thin_points = thin_component_points(&component);
        if thin_points.is_empty() {
            continue;
        }

        let chain = order_component_points(&thin_points);
        if !chain.is_empty() {
            chains.push(chain);
        }
    }

    if chains.is_empty() {
        vec![order_component_points(pixels)]
    } else {
        chains
    }
}

fn thin_component_points(component: &[(u32, u32)]) -> Vec<(u32, u32)> {
    if component.len() <= 2 {
        let axis = dominant_axis(component);
        let mut points = component.to_vec();
        points.sort_by(|a, b| compare_points_along_axis(*a, *b, axis));
        return points;
    }

    match dominant_axis(component) {
        DominantAxis::Horizontal => {
            let mut by_x: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for &(x, y) in component {
                by_x.entry(x).or_default().push(y);
            }

            by_x.into_iter()
                .map(|(x, mut ys)| {
                    ys.sort_unstable();
                    (x, ys[ys.len() / 2])
                })
                .collect()
        }
        DominantAxis::Vertical => {
            let mut by_y: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for &(x, y) in component {
                by_y.entry(y).or_default().push(x);
            }

            by_y.into_iter()
                .map(|(y, mut xs)| {
                    xs.sort_unstable();
                    (xs[xs.len() / 2], y)
                })
                .collect()
        }
    }
}

fn order_component_points(points: &[(u32, u32)]) -> Vec<(u32, u32)> {
    if points.is_empty() {
        return Vec::new();
    }

    let axis = dominant_axis(points);
    let mut remaining = points.to_vec();
    remaining.sort_by(|a, b| compare_points_along_axis(*a, *b, axis));

    let mut chain = Vec::with_capacity(remaining.len());
    let mut current = remaining.remove(0);
    chain.push(current);

    while !remaining.is_empty() {
        let best_idx = remaining
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                point_distance_sq(current, **a).cmp(&point_distance_sq(current, **b))
            })
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        current = remaining.swap_remove(best_idx);
        chain.push(current);
    }

    chain
}
#[cfg(test)]
mod tests {
    use super::{build_pixel_chains, sample_points_from_cluster};

    #[test]
    fn build_pixel_chains_preserves_vertical_components() {
        let pixels: Vec<(u32, u32)> = (0..8).map(|y| (10, y)).collect();
        let chains = build_pixel_chains(&pixels);

        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].len(), pixels.len());
        assert_eq!(chains[0].first(), Some(&(10, 0)));
        assert_eq!(chains[0].last(), Some(&(10, 7)));
    }

    #[test]
    fn build_pixel_chains_keeps_disconnected_components_separate() {
        let mut pixels = vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)];
        pixels.extend([(20, 10), (21, 10), (22, 10), (20, 11), (21, 11), (22, 11)]);

        let chains = build_pixel_chains(&pixels);
        assert_eq!(chains.len(), 2);

        let min_xs: Vec<u32> = chains
            .iter()
            .map(|chain| chain.iter().map(|&(x, _)| x).min().unwrap_or(0))
            .collect();
        assert!(min_xs.contains(&0));
        assert!(min_xs.contains(&20));
    }

    #[test]
    fn sample_points_from_cluster_keeps_vertical_span() {
        let pixels: Vec<(u32, u32)> = (0..20).map(|y| (5, y)).collect();
        let sampled = sample_points_from_cluster(&pixels, 5, 64);

        assert_eq!(sampled.len(), 5);

        let min_y = sampled
            .iter()
            .map(|&(_, y)| y)
            .fold(f32::INFINITY, f32::min);
        let max_y = sampled
            .iter()
            .map(|&(_, y)| y)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(min_y <= 1.0);
        assert!(max_y >= 18.0);
    }
}
