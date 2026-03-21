use super::{image::LinearRgbaImage, pyramid::BlurLevel};
use crate::core::EPSILON;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SigmaSample {
  Single(usize),
  Blend { low: usize, high: usize, t: f32 },
}

pub(crate) fn sample_sigma(
  sigmas: &[f32],
  levels: &[BlurLevel],
  x: usize,
  y: usize,
  sigma: f32,
) -> [f32; 4] {
  match resolve_sigma_sample(sigmas, sigma) {
    SigmaSample::Single(index) => sample_level_at_base(&levels[index], x, y),
    SigmaSample::Blend { low, high, t } => {
      let low = sample_level_at_base(&levels[low], x, y);
      let high = sample_level_at_base(&levels[high], x, y);
      let mut out = [0.0; 4];

      for channel in 0..4 {
        out[channel] = low[channel] + (high[channel] - low[channel]) * t;
      }

      out
    }
  }
}

pub(crate) fn resolve_sigma_sample(sigmas: &[f32], sigma: f32) -> SigmaSample {
  if sigmas.len() <= 1 || sigma <= sigmas[0] + EPSILON {
    return SigmaSample::Single(0);
  }

  let last_index = sigmas.len() - 1;
  if sigma >= sigmas[last_index] - EPSILON {
    return SigmaSample::Single(last_index);
  }

  let hi = sigmas.partition_point(|value| *value < sigma);
  let lo = hi.saturating_sub(1);
  let low_sigma = sigmas[lo];
  let high_sigma = sigmas[hi];

  if (high_sigma - low_sigma).abs() < EPSILON {
    SigmaSample::Single(lo)
  } else {
    let t = ((sigma - low_sigma) / (high_sigma - low_sigma)).clamp(0.0, 1.0);
    SigmaSample::Blend {
      low: lo,
      high: hi,
      t,
    }
  }
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
