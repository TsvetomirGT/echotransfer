//! Cover art normalization for Snowsky Echo players: small, baseline JPEG.

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

use crate::error::Result;

const QUALITY: u8 = 90;

/// Decode any supported image, downscale so the longest edge is <= `max_edge`,
/// re-encode as baseline (non-progressive) RGB JPEG.
pub fn normalize(bytes: &[u8], max_edge: u32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes)?;
    let img = if img.width() > max_edge || img.height() > max_edge {
        img.resize(max_edge, max_edge, FilterType::Lanczos3)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut out = Vec::with_capacity(64 * 1024);
    JpegEncoder::new_with_quality(&mut out, QUALITY).encode_image(&rgb)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    pub fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x % 255) as u8, (y % 255) as u8, 128, 255]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    /// True if JPEG contains an SOF2 (progressive) marker.
    fn is_progressive(jpeg: &[u8]) -> bool {
        jpeg.windows(2).any(|w| w == [0xFF, 0xC2])
    }

    #[test]
    fn downsizes_and_converts_png_to_baseline_jpeg() {
        let out = normalize(&png(1200, 800), 500).unwrap();
        assert_eq!(&out[..2], &[0xFF, 0xD8]);
        assert!(!is_progressive(&out));
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (500, 333));
    }

    #[test]
    fn keeps_small_images() {
        let out = normalize(&png(300, 300), 500).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (300, 300));
    }

    #[test]
    fn rejects_garbage() {
        assert!(normalize(b"not an image", 500).is_err());
    }
}
