use std::collections::HashMap;

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

    let (best_q, _) = histogram
        .iter()
        .max_by_key(|&(_, &c)| c)
        .unwrap_or((&(7, 7, 7), &0));

    [
        (best_q.0 << 5) | 0x10,
        (best_q.1 << 5) | 0x10,
        (best_q.2 << 5) | 0x10,
    ]
}

pub fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> f32 {
    let dr = a[0] as f32 - b[0] as f32;
    let dg = a[1] as f32 - b[1] as f32;
    let db = a[2] as f32 - b[2] as f32;
    dr * dr + dg * dg + db * db
}

pub fn is_bg_color(pixel: [u8; 3], bg: [u8; 3]) -> bool {
    color_distance_sq(pixel, bg) < 30.0 * 30.0 * 3.0
}

pub fn collect_masked_non_bg_pixels(
    rgba: &[u8],
    mask: &[bool],
    w: u32,
    h: u32,
    bg_color: [u8; 3],
) -> Vec<([u8; 3], u32, u32)> {
    let w_us = w as usize;
    let h_us = h as usize;
    let mut pixels = Vec::new();

    for y in 0..h_us {
        for x in 0..w_us {
            let idx = y * w_us + x;
            if !mask.get(idx).copied().unwrap_or(false) {
                continue;
            }

            let off = idx * 4;
            if off + 2 >= rgba.len() {
                continue;
            }

            let pixel = [rgba[off], rgba[off + 1], rgba[off + 2]];
            if is_bg_color(pixel, bg_color) {
                continue;
            }

            pixels.push((pixel, x as u32, y as u32));
        }
    }

    pixels
}
