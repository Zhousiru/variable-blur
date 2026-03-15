use image::DynamicImage;
use rayon::prelude::*;

use crate::core::{
  domain::{normalize_direction, BlurCurve, DirectionalBlurOptions, VariableBlurConfig},
  engine::{
    image::LinearRgbaImage,
    pyramid::{build_blur_levels_pyramid, BlurLevel},
    sampling::sample_sigma,
    schedule::generate_sigmas,
  },
  io::codec::{from_dynamic_image, to_dynamic_image},
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
    let sigmas = generate_sigmas(&cfg.schedule, cfg.steps, cfg.max_sigma);
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
    let inv_span = if (end - start).abs() < EPSILON {
      0.0
    } else {
      1.0 / (end - start)
    };
    let mut out = LinearRgbaImage::new(self.width, self.height);

    out
      .data
      .par_chunks_mut(self.width)
      .enumerate()
      .for_each(|(y, row)| {
        for (x, pixel) in row.iter_mut().enumerate() {
          let projection = (x as f32 + 0.5) * dir[0] + (y as f32 + 0.5) * dir[1];
          let t = if inv_span == 0.0 {
            if projection >= end {
              1.0
            } else {
              0.0
            }
          } else {
            ((projection - start) * inv_span).clamp(0.0, 1.0)
          };
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
