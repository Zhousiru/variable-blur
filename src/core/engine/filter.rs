use rayon::prelude::*;

use super::{image::LinearRgbaImage, sampling::clamp_index};
use crate::core::EPSILON;

pub(crate) fn blur_separable(src: &LinearRgbaImage, sigma: f32) -> LinearRgbaImage {
  if sigma <= EPSILON {
    return src.clone();
  }

  let kernel = gaussian_kernel(sigma);
  let radius = (kernel.len() / 2) as isize;
  let mut horizontal = LinearRgbaImage::new(src.width, src.height);

  horizontal
    .data
    .par_chunks_mut(src.width)
    .enumerate()
    .for_each(|(y, row)| {
      for (x, pixel) in row.iter_mut().enumerate() {
        let mut acc = [0.0; 4];
        for (offset, weight) in kernel.iter().enumerate() {
          let sx = clamp_index(x as isize + offset as isize - radius, src.width);
          let sample = src.data[y * src.width + sx];
          for channel in 0..4 {
            acc[channel] += sample[channel] * *weight;
          }
        }
        *pixel = acc;
      }
    });

  let mut output = LinearRgbaImage::new(src.width, src.height);
  output
    .data
    .par_chunks_mut(src.width)
    .enumerate()
    .for_each(|(y, row)| {
      for (x, pixel) in row.iter_mut().enumerate() {
        let mut acc = [0.0; 4];
        for (offset, weight) in kernel.iter().enumerate() {
          let sy = clamp_index(y as isize + offset as isize - radius, src.height);
          let sample = horizontal.data[sy * src.width + x];
          for channel in 0..4 {
            acc[channel] += sample[channel] * *weight;
          }
        }
        *pixel = acc;
      }
    });

  output
}

pub(crate) fn downsample_half_box(src: &LinearRgbaImage) -> LinearRgbaImage {
  let width = src.width.div_ceil(2).max(1);
  let height = src.height.div_ceil(2).max(1);
  let mut out = LinearRgbaImage::new(width, height);

  for y in 0..height {
    for x in 0..width {
      let mut acc = [0.0; 4];
      for oy in 0..2 {
        for ox in 0..2 {
          let sx = (x * 2 + ox).min(src.width - 1);
          let sy = (y * 2 + oy).min(src.height - 1);
          let sample = src.data[sy * src.width + sx];
          for channel in 0..4 {
            acc[channel] += sample[channel];
          }
        }
      }
      for value in &mut acc {
        *value *= 0.25;
      }
      out.data[y * width + x] = acc;
    }
  }

  out
}

fn gaussian_kernel(sigma: f32) -> Vec<f32> {
  let radius = (sigma * 3.0).ceil().max(1.0) as usize;
  let mut kernel = Vec::with_capacity(radius * 2 + 1);
  let mut sum = 0.0;

  for i in 0..=(radius * 2) {
    let x = i as f32 - radius as f32;
    let value = (-0.5 * x * x / (sigma * sigma)).exp();
    kernel.push(value);
    sum += value;
  }

  for value in &mut kernel {
    *value /= sum;
  }

  kernel
}
