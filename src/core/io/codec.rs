use std::{
  error::Error,
  fmt::{self, Display, Formatter},
  io::Cursor,
};

use fast_srgb8::{f32x4_to_srgb8, srgb8_to_f32};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, RgbaImage};
use rayon::prelude::*;

use crate::core::{engine::image::LinearRgbaImage, EPSILON};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RawImageError {
  ZeroDimensions,
  UnsupportedChannels(usize),
  BufferLengthMismatch { expected: usize, actual: usize },
  ImageTooLarge,
}

impl Display for RawImageError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Self::ZeroDimensions => f.write_str("raw image width and height must be > 0"),
      Self::UnsupportedChannels(channels) => {
        write!(f, "raw image channels must be 3 or 4, got {channels}")
      }
      Self::BufferLengthMismatch { expected, actual } => write!(
        f,
        "raw image buffer length mismatch: expected {expected} bytes, got {actual}"
      ),
      Self::ImageTooLarge => f.write_str("raw image dimensions are too large"),
    }
  }
}

impl Error for RawImageError {}

pub(crate) fn from_dynamic_image(image: &DynamicImage) -> LinearRgbaImage {
  let rgba = image.to_rgba8();
  let (width, height) = rgba.dimensions();
  decode_raw_pixels(rgba.as_raw(), width as usize, height as usize, 4)
}

pub(crate) fn to_dynamic_image(image: &LinearRgbaImage) -> DynamicImage {
  let raw = encode_raw_pixels(image, 4);

  DynamicImage::ImageRgba8(
    RgbaImage::from_raw(image.width as u32, image.height as u32, raw)
      .expect("rgba buffer size matches image dimensions"),
  )
}

pub(crate) fn from_raw_pixels(
  raw: &[u8],
  width: usize,
  height: usize,
  channels: usize,
) -> Result<LinearRgbaImage, RawImageError> {
  validate_raw_layout(raw.len(), width, height, channels)?;
  Ok(decode_raw_pixels(raw, width, height, channels))
}

pub(crate) fn to_raw_pixels(
  image: &LinearRgbaImage,
  channels: usize,
) -> Result<Vec<u8>, RawImageError> {
  validate_dimensions(image.width, image.height)?;
  validate_channels(channels)?;
  Ok(encode_raw_pixels(image, channels))
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

fn decode_raw_pixels(raw: &[u8], width: usize, height: usize, channels: usize) -> LinearRgbaImage {
  let data = match channels {
    3 => raw
      .par_chunks_exact(3)
      .map(|pixel| {
        let rgb = [
          srgb_to_linear(pixel[0]),
          srgb_to_linear(pixel[1]),
          srgb_to_linear(pixel[2]),
        ];
        [rgb[0], rgb[1], rgb[2], 1.0]
      })
      .collect(),
    4 => raw
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
      .collect(),
    _ => unreachable!("raw channels validated before decode"),
  };

  LinearRgbaImage {
    width,
    height,
    data,
  }
}

fn encode_raw_pixels(image: &LinearRgbaImage, channels: usize) -> Vec<u8> {
  let mut raw = vec![0u8; image.width * image.height * channels];

  match channels {
    3 => {
      raw
        .par_chunks_exact_mut(3)
        .zip(image.data.par_iter())
        .for_each(|(pixel, src)| {
          let alpha = src[3].clamp(0.0, 1.0);
          let inv_alpha = if alpha > EPSILON { 1.0 / alpha } else { 0.0 };
          let [r, g, b, _] =
            linear_rgb_to_srgb([src[0] * inv_alpha, src[1] * inv_alpha, src[2] * inv_alpha]);
          pixel[0] = r;
          pixel[1] = g;
          pixel[2] = b;
        });
    }
    4 => {
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
    }
    _ => unreachable!("raw channels validated before encode"),
  }

  raw
}

fn validate_raw_layout(
  actual_len: usize,
  width: usize,
  height: usize,
  channels: usize,
) -> Result<(), RawImageError> {
  validate_dimensions(width, height)?;
  validate_channels(channels)?;

  let expected = expected_raw_len(width, height, channels)?;
  if actual_len == expected {
    Ok(())
  } else {
    Err(RawImageError::BufferLengthMismatch {
      expected,
      actual: actual_len,
    })
  }
}

fn validate_dimensions(width: usize, height: usize) -> Result<(), RawImageError> {
  if width == 0 || height == 0 {
    Err(RawImageError::ZeroDimensions)
  } else {
    Ok(())
  }
}

fn validate_channels(channels: usize) -> Result<(), RawImageError> {
  match channels {
    3 | 4 => Ok(()),
    _ => Err(RawImageError::UnsupportedChannels(channels)),
  }
}

fn expected_raw_len(width: usize, height: usize, channels: usize) -> Result<usize, RawImageError> {
  width
    .checked_mul(height)
    .and_then(|pixels| pixels.checked_mul(channels))
    .ok_or(RawImageError::ImageTooLarge)
}

#[cfg(test)]
mod tests {
  use super::{from_raw_pixels, to_raw_pixels, RawImageError};

  #[test]
  fn raw_rgb_input_is_decoded_as_opaque() {
    let image = from_raw_pixels(&[255, 128, 0], 1, 1, 3).expect("valid rgb input");

    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    assert_eq!(image.data.len(), 1);
    assert!((image.data[0][3] - 1.0).abs() < 1e-6);
  }

  #[test]
  fn raw_pixels_reject_invalid_buffer_length() {
    assert!(matches!(
      from_raw_pixels(&[0, 1, 2], 2, 1, 4),
      Err(RawImageError::BufferLengthMismatch {
        expected: 8,
        actual: 3,
      })
    ));
  }

  #[test]
  fn raw_pixels_roundtrip_preserves_channel_count() {
    let image = from_raw_pixels(&[255, 0, 0, 128], 1, 1, 4).expect("valid rgba input");
    let encoded = to_raw_pixels(&image, 4).expect("valid rgba output");

    assert_eq!(encoded.len(), 4);
    assert_eq!(encoded[3], 128);
  }
}
