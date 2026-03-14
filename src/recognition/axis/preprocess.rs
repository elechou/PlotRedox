use super::super::pixels::is_bg_color;
use image::GrayImage;

pub(super) fn build_foreground_image(
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

pub(super) fn collect_active_pixels(img: &GrayImage) -> Vec<(f32, f32)> {
    let (w, h) = img.dimensions();
    let raw = img.as_raw();
    let wu = w as usize;
    let mut active_pixels = Vec::with_capacity(8192);

    for y in 0..h {
        let row = y as usize * wu;
        for x in 0..w {
            if raw[row + x as usize] > 0 {
                active_pixels.push((x as f32, y as f32));
            }
        }
    }

    active_pixels
}

pub(super) fn directional_open(img: &GrayImage, kw: u32, kh: u32) -> GrayImage {
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

fn erode_rows(img: &GrayImage, k: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let half = (k / 2) as usize;
    let wu = w as usize;
    let mut dst = vec![0u8; src.len()];

    let mut pfx = vec![0u32; wu + 1];
    for y in 0..h as usize {
        let row = y * wu;
        pfx[0] = 0;
        for x in 0..wu {
            pfx[x + 1] = pfx[x] + u32::from(src[row + x] == 0);
        }
        for x in 0..wu {
            let lo = x.saturating_sub(half);
            let hi = (x + half).min(wu - 1);
            if pfx[hi + 1] == pfx[lo] {
                dst[row + x] = 255;
            }
        }
    }
    GrayImage::from_raw(w, h, dst).unwrap()
}

fn dilate_rows(img: &GrayImage, k: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let half = (k / 2) as usize;
    let wu = w as usize;
    let mut dst = vec![0u8; src.len()];

    let mut pfx = vec![0u32; wu + 1];
    for y in 0..h as usize {
        let row = y * wu;
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
