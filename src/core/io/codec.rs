use std::io::Cursor;

use fast_srgb8::{f32x4_to_srgb8, srgb8_to_f32};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, RgbaImage};
use rayon::prelude::*;

use crate::core::{engine::image::LinearRgbaImage, EPSILON};

pub(crate) fn from_dynamic_image(image: &DynamicImage) -> LinearRgbaImage {
  let rgba = image.to_rgba8();
  let (width, height) = rgba.dimensions();
  let data = rgba
    .as_raw()
    .par_chunks_exact(4)
    .map(|pixel| {
      let alpha = pixel[3] as f32 / 255.0;
      let rgb = [
        srgb_to_linear(pixel[0]),
        srgb_to_linear(pixel[1]),
        srgb_to_linear(pixel[2]),
      ];
      [rgb[0] * alpha, rgb[1] * alpha, rgb[2] * alpha, alpha]
    })
    .collect();

  LinearRgbaImage {
    width: width as usize,
    height: height as usize,
    data,
  }
}

pub(crate) fn to_dynamic_image(image: &LinearRgbaImage) -> DynamicImage {
  let mut raw = vec![0u8; image.width * image.height * 4];

  raw
    .par_chunks_exact_mut(4)
    .zip(image.data.par_iter())
    .for_each(|(pixel, src)| {
      let alpha = src[3].clamp(0.0, 1.0);
      let inv_alpha = if alpha > EPSILON { 1.0 / alpha } else { 0.0 };
      let [r, g, b, _] =
        linear_rgb_to_srgb([src[0] * inv_alpha, src[1] * inv_alpha, src[2] * inv_alpha]);
      pixel[0] = r;
      pixel[1] = g;
      pixel[2] = b;
      pixel[3] = (alpha * 255.0).round() as u8;
    });

  DynamicImage::ImageRgba8(
    RgbaImage::from_raw(image.width as u32, image.height as u32, raw)
      .expect("rgba buffer size matches image dimensions"),
  )
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
  srgb8_to_f32(value)
}

fn linear_rgb_to_srgb(rgb: [f32; 3]) -> [u8; 4] {
  f32x4_to_srgb8([rgb[0], rgb[1], rgb[2], 0.0])
}
