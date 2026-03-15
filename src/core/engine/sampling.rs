use super::{image::LinearRgbaImage, pyramid::BlurLevel};
use crate::core::EPSILON;

pub(crate) fn sample_sigma(
  sigmas: &[f32],
  levels: &[BlurLevel],
  x: usize,
  y: usize,
  sigma: f32,
) -> [f32; 4] {
  if sigma <= sigmas[0] + EPSILON {
    return sample_level_at_base(&levels[0], x, y);
  }

  let last_index = sigmas.len() - 1;
  if sigma >= sigmas[last_index] - EPSILON {
    return sample_level_at_base(&levels[last_index], x, y);
  }

  let hi = sigmas.partition_point(|value| *value < sigma);
  let lo = hi.saturating_sub(1);
  let low_sigma = sigmas[lo];
  let high_sigma = sigmas[hi];

  if (high_sigma - low_sigma).abs() < EPSILON {
    return sample_level_at_base(&levels[lo], x, y);
  }

  let t = ((sigma - low_sigma) / (high_sigma - low_sigma)).clamp(0.0, 1.0);
  let low = sample_level_at_base(&levels[lo], x, y);
  let high = sample_level_at_base(&levels[hi], x, y);
  let mut out = [0.0; 4];

  for channel in 0..4 {
    out[channel] = low[channel] + (high[channel] - low[channel]) * t;
  }

  out
}

fn sample_level_at_base(level: &BlurLevel, x: usize, y: usize) -> [f32; 4] {
  let scale = (1usize << level.pyramid_level) as f32;
  let fx = ((x as f32 + 0.5) / scale) - 0.5;
  let fy = ((y as f32 + 0.5) / scale) - 0.5;
  bilinear_sample(&level.image, fx, fy)
}

fn bilinear_sample(image: &LinearRgbaImage, x: f32, y: f32) -> [f32; 4] {
  let x0 = x.floor() as isize;
  let y0 = y.floor() as isize;
  let x1 = x0 + 1;
  let y1 = y0 + 1;
  let tx = x - x.floor();
  let ty = y - y.floor();

  let c00 = image.data[clamp_index(y0, image.height) * image.width + clamp_index(x0, image.width)];
  let c10 = image.data[clamp_index(y0, image.height) * image.width + clamp_index(x1, image.width)];
  let c01 = image.data[clamp_index(y1, image.height) * image.width + clamp_index(x0, image.width)];
  let c11 = image.data[clamp_index(y1, image.height) * image.width + clamp_index(x1, image.width)];

  let mut out = [0.0; 4];
  for channel in 0..4 {
    let top = c00[channel] + (c10[channel] - c00[channel]) * tx;
    let bottom = c01[channel] + (c11[channel] - c01[channel]) * tx;
    out[channel] = top + (bottom - top) * ty;
  }

  out
}

pub(crate) fn clamp_index(value: isize, len: usize) -> usize {
  value.clamp(0, (len.saturating_sub(1)) as isize) as usize
}
