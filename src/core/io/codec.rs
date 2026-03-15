use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

use crate::core::{engine::image::LinearRgbaImage, EPSILON};

pub(crate) fn from_dynamic_image(image: &DynamicImage) -> LinearRgbaImage {
  let rgba = image.to_rgba8();
  let (width, height) = rgba.dimensions();
  let mut out = LinearRgbaImage::new(width as usize, height as usize);

  for (index, pixel) in rgba.pixels().enumerate() {
    let alpha = pixel[3] as f32 / 255.0;
    let rgb = [
      srgb_to_linear(pixel[0]),
      srgb_to_linear(pixel[1]),
      srgb_to_linear(pixel[2]),
    ];
    out.data[index] = [rgb[0] * alpha, rgb[1] * alpha, rgb[2] * alpha, alpha];
  }

  out
}

pub(crate) fn to_dynamic_image(image: &LinearRgbaImage) -> DynamicImage {
  let mut rgba = RgbaImage::new(image.width as u32, image.height as u32);

  for (index, pixel) in rgba.pixels_mut().enumerate() {
    let src = image.data[index];
    let alpha = src[3].clamp(0.0, 1.0);
    let inv_alpha = if alpha > EPSILON { 1.0 / alpha } else { 0.0 };
    let rgb = [
      linear_to_srgb(src[0] * inv_alpha),
      linear_to_srgb(src[1] * inv_alpha),
      linear_to_srgb(src[2] * inv_alpha),
    ];
    *pixel = Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]);
  }

  DynamicImage::ImageRgba8(rgba)
}

pub fn encode_dynamic_image(
  image: &DynamicImage,
  requested_format: Option<ImageFormat>,
  fallback: ImageFormat,
) -> image::ImageResult<Vec<u8>> {
  let format = requested_format.unwrap_or(match fallback {
    ImageFormat::Png
    | ImageFormat::Jpeg
    | ImageFormat::WebP
    | ImageFormat::Bmp
    | ImageFormat::Tiff
    | ImageFormat::Tga => fallback,
    _ => ImageFormat::Png,
  });

  let mut cursor = Cursor::new(Vec::new());
  match format {
    ImageFormat::Jpeg => {
      let rgb = rgba_to_rgb(image.to_rgba8());
      DynamicImage::ImageRgb8(rgb).write_to(&mut cursor, format)?;
    }
    _ => {
      image.write_to(&mut cursor, format)?;
    }
  }

  Ok(cursor.into_inner())
}

fn rgba_to_rgb(rgba: RgbaImage) -> RgbImage {
  let mut rgb = RgbImage::new(rgba.width(), rgba.height());
  for (src, dst) in rgba.pixels().zip(rgb.pixels_mut()) {
    *dst = Rgb([src[0], src[1], src[2]]);
  }
  rgb
}

fn srgb_to_linear(value: u8) -> f32 {
  let srgb = value as f32 / 255.0;
  if srgb <= 0.04045 {
    srgb / 12.92
  } else {
    ((srgb + 0.055) / 1.055).powf(2.4)
  }
}

fn linear_to_srgb(value: f32) -> u8 {
  let linear = value.clamp(0.0, 1.0);
  let srgb = if linear <= 0.0031308 {
    linear * 12.92
  } else {
    1.055 * linear.powf(1.0 / 2.4) - 0.055
  };
  (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}
