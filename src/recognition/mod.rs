pub mod axis;
pub mod data;
mod geometry;
pub mod grid_removal;
pub mod mask;
mod pixels;
mod spatial;

pub use self::pixels::detect_background_color;

/// Apply morphological closing (dilate then erode) to a boolean mask buffer.
/// Bridges small gaps (up to `2 * radius` pixels) in painted mask regions.
#[allow(dead_code)]
pub fn close_mask(buffer: &[bool], w: u32, h: u32, radius: u8) -> Vec<bool> {
    use image::GrayImage;
    use imageproc::distance_transform::Norm;
    use imageproc::morphology::close;

    let mut img = GrayImage::new(w, h);
    for (i, &val) in buffer.iter().enumerate() {
        if val {
            img.as_mut()[i] = 255;
        }
    }

    let closed = close(&img, Norm::LInf, radius);

    closed.into_raw().iter().map(|&v| v > 128).collect()
}
