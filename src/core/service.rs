use fast_srgb8::srgb8_to_f32;
use image::DynamicImage;
use rayon::prelude::*;

use crate::core::{
  domain::{
    generate_sigma_anchors, normalize_direction, BlurCurve, DirectionalBlurOptions,
    VariableBlurConfig,
  },
  engine::{
    image::LinearRgbaImage,
    pyramid::{build_blur_levels_pyramid, BlurLevel},
    sampling::{resolve_sigma_sample, sample_sigma, SigmaSample},
  },
  io::codec::{
    from_dynamic_image, from_raw_pixels, to_dynamic_image, to_raw_pixels, RawImageError,
  },
  EPSILON,
};

pub(crate) struct PreparedVariableBlur {
  width: usize,
  height: usize,
  max_sigma: f32,
  curve: BlurCurve,
  sigmas: Vec<f32>,
  levels: Vec<BlurLevel>,
}

impl PreparedVariableBlur {
  pub(crate) fn new(src: &LinearRgbaImage, cfg: VariableBlurConfig) -> Self {
    let sigmas = generate_sigma_anchors(&cfg.curve, cfg.steps, cfg.max_sigma);
    let levels = build_blur_levels_pyramid(src, &sigmas, &cfg.pyramid);

    Self {
      width: src.width,
      height: src.height,
      max_sigma: cfg.max_sigma,
      curve: cfg.curve,
      sigmas,
      levels,
    }
  }

  pub(crate) fn apply_with_direction(
    &self,
    direction: [f32; 2],
    start: f32,
    end: f32,
  ) -> LinearRgbaImage {
    let dir = normalize_direction(direction);
    let mut out = LinearRgbaImage::new(self.width, self.height);

    out
      .data
      .par_chunks_mut(self.width)
      .enumerate()
      .for_each(|(y, row)| {
        for (x, pixel) in row.iter_mut().enumerate() {
          let projection = (x as f32 + 0.5) * dir[0] + (y as f32 + 0.5) * dir[1];
          let t = directional_progress(projection, start, end).clamp(0.0, 1.0);
          let sigma = self.curve.eval(t) * self.max_sigma;
          *pixel = sample_sigma(&self.sigmas, &self.levels, x, y, sigma);
        }
      });

    out
  }
}

pub fn apply_directional_variable_blur(
  image: &DynamicImage,
  cfg: VariableBlurConfig,
  options: DirectionalBlurOptions,
) -> DynamicImage {
  let src = from_dynamic_image(image);
  let prepared = PreparedVariableBlur::new(&src, cfg);
  let output = prepared.apply_with_direction(options.direction, options.start, options.end);
  to_dynamic_image(&output)
}

pub fn generate_directional_step_map(
  dimensions: (u32, u32),
  cfg: VariableBlurConfig,
  options: DirectionalBlurOptions,
) -> DynamicImage {
  let width = dimensions.0 as usize;
  let height = dimensions.1 as usize;
  let dir = normalize_direction(options.direction);
  let sigmas = generate_sigma_anchors(&cfg.curve, cfg.steps, cfg.max_sigma);
  let colors = generate_step_colors(sigmas.len());
  let mut out = LinearRgbaImage::new(width, height);

  out
    .data
    .par_chunks_mut(width.max(1))
    .enumerate()
    .for_each(|(y, row)| {
      for (x, pixel) in row.iter_mut().enumerate() {
        let projection = (x as f32 + 0.5) * dir[0] + (y as f32 + 0.5) * dir[1];
        let raw_t = directional_progress(projection, options.start, options.end);
        if !(0.0..=1.0).contains(&raw_t) {
          *pixel = [0.0, 0.0, 0.0, 1.0];
          continue;
        }

        let sigma = cfg.curve.eval(raw_t) * cfg.max_sigma;
        *pixel = sample_step_color(&sigmas, &colors, sigma);
      }
    });

  to_dynamic_image(&out)
}

pub fn apply_directional_variable_blur_raw(
  raw: &[u8],
  width: u32,
  height: u32,
  channels: u32,
  cfg: VariableBlurConfig,
  options: DirectionalBlurOptions,
) -> Result<Vec<u8>, RawImageError> {
  let src = from_raw_pixels(raw, width as usize, height as usize, channels as usize)?;
  let prepared = PreparedVariableBlur::new(&src, cfg);
  let output = prepared.apply_with_direction(options.direction, options.start, options.end);
  to_raw_pixels(&output, channels as usize)
}

fn directional_progress(projection: f32, start: f32, end: f32) -> f32 {
  let span = end - start;
  if span.abs() < EPSILON {
    if projection >= end {
      1.0
    } else {
      -1.0
    }
  } else {
    (projection - start) / span
  }
}

fn sample_step_color(sigmas: &[f32], colors: &[[f32; 4]], sigma: f32) -> [f32; 4] {
  match resolve_sigma_sample(sigmas, sigma) {
    SigmaSample::Single(index) => colors[index],
    SigmaSample::Blend { low, high, t } => blend_rgba(colors[low], colors[high], t),
  }
}

fn blend_rgba(low: [f32; 4], high: [f32; 4], t: f32) -> [f32; 4] {
  let mut out = [0.0; 4];
  for channel in 0..4 {
    out[channel] = low[channel] + (high[channel] - low[channel]) * t;
  }
  out
}

fn generate_step_colors(count: usize) -> Vec<[f32; 4]> {
  (0..count).map(step_color).collect()
}

fn step_color(index: usize) -> [f32; 4] {
  let mut state = 0xA076_1D64_78BD_642Fu64 ^ (index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
  let hue_jitter = next_unit_f32(&mut state) * 0.18 - 0.09;
  let hue = ((index as f32 * 0.618_034) + hue_jitter).rem_euclid(1.0);
  let saturation = 0.65 + next_unit_f32(&mut state) * 0.25;
  let value = 0.78 + next_unit_f32(&mut state) * 0.2;
  let rgb = hsv_to_rgb(hue, saturation.min(1.0), value.min(1.0));
  let srgb = rgb.map(|channel| (channel * 255.0).round() as u8);

  [
    srgb8_to_f32(srgb[0]),
    srgb8_to_f32(srgb[1]),
    srgb8_to_f32(srgb[2]),
    1.0,
  ]
}

fn next_unit_f32(state: &mut u64) -> f32 {
  let bits = (splitmix64(state) >> 40) as u32;
  bits as f32 / ((1u32 << 24) as f32)
}

fn splitmix64(state: &mut u64) -> u64 {
  *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
  let mut z = *state;
  z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
  z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
  z ^ (z >> 31)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
  let hue = hue.rem_euclid(1.0) * 6.0;
  let chroma = value * saturation;
  let x = chroma * (1.0 - ((hue.rem_euclid(2.0)) - 1.0).abs());
  let (r1, g1, b1) = match hue.floor() as i32 {
    0 => (chroma, x, 0.0),
    1 => (x, chroma, 0.0),
    2 => (0.0, chroma, x),
    3 => (0.0, x, chroma),
    4 => (x, 0.0, chroma),
    _ => (chroma, 0.0, x),
  };
  let m = value - chroma;
  [r1 + m, g1 + m, b1 + m]
}

#[cfg(test)]
mod tests {
  use crate::core::{
    domain::{BlurCurve, DirectionalBlurOptions, PyramidConfig, VariableBlurConfig},
    engine::image::LinearRgbaImage,
  };

  use super::{
    generate_directional_step_map, generate_step_colors, sample_step_color, to_dynamic_image,
  };

  fn test_config() -> VariableBlurConfig {
    VariableBlurConfig {
      max_sigma: 8.0,
      steps: 4,
      curve: BlurCurve::Linear,
      pyramid: PyramidConfig {
        max_levels: 1,
        target_local_sigma: 1.0,
        min_local_sigma: 0.5,
        max_local_sigma: 2.0,
        downsample_stage_sigma: 0.5,
      },
    }
  }

  #[test]
  fn step_map_is_black_outside_active_span() {
    let map = generate_directional_step_map(
      (4, 1),
      test_config(),
      DirectionalBlurOptions {
        direction: [1.0, 0.0],
        start: 1.0,
        end: 3.0,
      },
    )
    .to_rgba8();

    assert_eq!(map.get_pixel(0, 0).0, [0, 0, 0, 255]);
    assert_eq!(map.get_pixel(3, 0).0, [0, 0, 0, 255]);
    assert_ne!(map.get_pixel(1, 0).0, [0, 0, 0, 255]);
  }

  #[test]
  fn step_map_uses_same_sigma_blend_as_runtime_sampling() {
    let config = test_config();
    let map = generate_directional_step_map(
      (1, 1),
      config.clone(),
      DirectionalBlurOptions {
        direction: [1.0, 0.0],
        start: 0.0,
        end: 2.0,
      },
    )
    .to_rgba8();

    let sigmas = crate::core::generate_sigma_anchors(&config.curve, config.steps, config.max_sigma);
    let colors = generate_step_colors(sigmas.len());
    let sigma = config.curve.eval(0.25) * config.max_sigma;
    let expected = sample_step_color(&sigmas, &colors, sigma);
    let expected_pixel = to_dynamic_image(&LinearRgbaImage {
      width: 1,
      height: 1,
      data: vec![expected],
    })
    .to_rgba8()
    .get_pixel(0, 0)
    .0;

    assert_eq!(map.get_pixel(0, 0).0, expected_pixel);
  }
}
