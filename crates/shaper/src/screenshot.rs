use std::io::Cursor;
use image::{imageops::FilterType, GenericImageView, ImageFormat};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScreenshotError {
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotOptions {
    pub scale: f32,
    pub region: Option<RegionCrop>,
    pub max_dimension: Option<u32>,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            scale: 0.5,
            region: None,
            max_dimension: Some(1024),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapedScreenshot {
    pub image_base64: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    pub scale_applied: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualDiffReport {
    pub match_percentage: f64,
    pub changed_pixels: usize,
    pub total_pixels: usize,
    pub bounding_box: Option<RegionCrop>,
    pub change_summary: String,
}

pub struct ScreenshotShaper;

impl ScreenshotShaper {
    /// Decodes a base64 encoded PNG or JPEG image and applies scaling and cropping rules.
    pub fn shape_base64(raw_base64: &str, opts: &ScreenshotOptions) -> Result<ShapedScreenshot, ScreenshotError> {
        let clean_b64 = raw_base64
            .trim()
            .trim_start_matches("data:image/png;base64,")
            .trim_start_matches("data:image/jpeg;base64,");

        let bytes = STANDARD.decode(clean_b64)?;
        Self::shape_bytes(&bytes, opts)
    }

    /// Processes raw image bytes, applying region cropping and proportional downscaling.
    pub fn shape_bytes(bytes: &[u8], opts: &ScreenshotOptions) -> Result<ShapedScreenshot, ScreenshotError> {
        let img = image::load_from_memory(bytes)?;
        let (orig_w, orig_h) = img.dimensions();

        let cropped = if let Some(ref r) = opts.region {
            let cx = r.x.min(orig_w.saturating_sub(1));
            let cy = r.y.min(orig_h.saturating_sub(1));
            let cw = r.width.min(orig_w - cx);
            let ch = r.height.min(orig_h - cy);
            img.crop_imm(cx, cy, cw, ch)
        } else {
            img
        };

        let (cw, ch) = cropped.dimensions();

        let target_scale = if opts.scale <= 0.0 || opts.scale > 1.0 {
            0.5
        } else {
            opts.scale
        };

        let mut target_w = ((cw as f32) * target_scale).round() as u32;
        let mut target_h = ((ch as f32) * target_scale).round() as u32;

        if let Some(max_dim) = opts.max_dimension {
            if target_w > max_dim || target_h > max_dim {
                let ratio = (max_dim as f32) / (target_w.max(target_h) as f32);
                target_w = ((target_w as f32) * ratio).round() as u32;
                target_h = ((target_h as f32) * ratio).round() as u32;
            }
        }

        target_w = target_w.max(1);
        target_h = target_h.max(1);

        let resized = cropped.resize_exact(target_w, target_h, FilterType::Triangle);

        let mut out_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut out_bytes);
        resized.write_to(&mut cursor, ImageFormat::Png)?;

        let b64 = STANDARD.encode(&out_bytes);

        Ok(ShapedScreenshot {
            image_base64: b64,
            format: "png".to_string(),
            width: target_w,
            height: target_h,
            original_width: orig_w,
            original_height: orig_h,
            scale_applied: target_scale,
        })
    }

    /// Performs daemon-side perceptual diffing between two images, returning a compact diff report.
    pub fn diff_images(bytes_a: &[u8], bytes_b: &[u8]) -> Result<VisualDiffReport, ScreenshotError> {
        let img_a = image::load_from_memory(bytes_a)?;
        let img_b = image::load_from_memory(bytes_b)?;

        let (w_a, h_a) = img_a.dimensions();
        let (w_b, h_b) = img_b.dimensions();

        if w_a != w_b || h_a != h_b {
            return Ok(VisualDiffReport {
                match_percentage: 0.0,
                changed_pixels: (w_a * h_a) as usize,
                total_pixels: (w_a * h_a) as usize,
                bounding_box: None,
                change_summary: format!("Resolution mismatch: {}x{} vs {}x{}", w_a, h_a, w_b, h_b),
            });
        }

        let total_pixels = (w_a * h_a) as usize;
        let mut diff_count = 0;
        let mut min_x = w_a;
        let mut max_x = 0;
        let mut min_y = h_a;
        let mut max_y = 0;

        let rgba_a = img_a.to_rgba8();
        let rgba_b = img_b.to_rgba8();

        for y in 0..h_a {
            for x in 0..w_a {
                let p_a = rgba_a.get_pixel(x, y);
                let p_b = rgba_b.get_pixel(x, y);

                // Per-channel color delta with tolerance threshold
                let dr = (p_a[0] as i32 - p_b[0] as i32).abs();
                let dg = (p_a[1] as i32 - p_b[1] as i32).abs();
                let db = (p_a[2] as i32 - p_b[2] as i32).abs();

                if dr + dg + db > 24 {
                    diff_count += 1;
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        let match_pct = if total_pixels > 0 {
            ((total_pixels - diff_count) as f64 / total_pixels as f64) * 100.0
        } else {
            100.0
        };

        let bbox = if diff_count > 0 && max_x >= min_x && max_y >= min_y {
            Some(RegionCrop {
                x: min_x,
                y: min_y,
                width: max_x - min_x + 1,
                height: max_y - min_y + 1,
            })
        } else {
            None
        };

        let summary = if diff_count == 0 {
            "Screenshots are visually identical (100.0% match)".to_string()
        } else {
            format!("Changes detected in {:.2}% of pixels localized to bounding box", 100.0 - match_pct)
        };

        Ok(VisualDiffReport {
            match_percentage: match_pct,
            changed_pixels: diff_count,
            total_pixels,
            bounding_box: bbox,
            change_summary: summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    fn create_test_image(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for x in 0..w {
            for y in 0..h {
                img.put_pixel(x, y, color);
            }
        }
        let mut buf = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn test_screenshot_downscaling_and_crop() {
        let raw_png = create_test_image(800, 600, Rgba([255, 0, 0, 255]));
        let opts = ScreenshotOptions {
            scale: 0.5,
            region: Some(RegionCrop {
                x: 100,
                y: 100,
                width: 400,
                height: 200,
            }),
            max_dimension: Some(1024),
        };

        let shaped = ScreenshotShaper::shape_bytes(&raw_png, &opts).unwrap();
        assert_eq!(shaped.original_width, 800);
        assert_eq!(shaped.original_height, 600);
        assert_eq!(shaped.width, 200);
        assert_eq!(shaped.height, 100);
        assert!(!shaped.image_base64.is_empty());
    }

    #[test]
    fn test_visual_diffing() {
        let img1 = create_test_image(100, 100, Rgba([0, 0, 0, 255]));
        let mut img2_raw = RgbaImage::new(100, 100);
        for x in 0..100 {
            for y in 0..100 {
                if x >= 20 && x <= 40 && y >= 20 && y <= 40 {
                    img2_raw.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                } else {
                    img2_raw.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
        let mut buf2 = Vec::new();
        DynamicImage::ImageRgba8(img2_raw)
            .write_to(&mut Cursor::new(&mut buf2), ImageFormat::Png)
            .unwrap();

        let report = ScreenshotShaper::diff_images(&img1, &buf2).unwrap();
        assert!(report.match_percentage > 90.0 && report.match_percentage < 100.0);
        assert!(report.bounding_box.is_some());
        let bbox = report.bounding_box.unwrap();
        assert_eq!(bbox.x, 20);
        assert_eq!(bbox.y, 20);
        assert_eq!(bbox.width, 21);
        assert_eq!(bbox.height, 21);
    }
}
