use super::{
  filter::{blur_separable, downsample_half_box},
  image::LinearRgbaImage,
};
use crate::core::{domain::PyramidConfig, EPSILON};

#[derive(Clone)]
struct PyramidLevel {
  image: LinearRgbaImage,
  base_sigma2_global: f32,
}

#[derive(Clone)]
pub(crate) struct BlurLevel {
  pub(crate) pyramid_level: usize,
  pub(crate) image: LinearRgbaImage,
}

pub(crate) fn build_blur_levels_pyramid(
  src: &LinearRgbaImage,
  sigmas: &[f32],
  cfg: &PyramidConfig,
) -> Vec<BlurLevel> {
  let pyramid = build_pyramid(src, cfg);
  let mut plans = Vec::with_capacity(sigmas.len());
  let mut grouped: Vec<Vec<(usize, f32)>> = vec![Vec::new(); pyramid.len()];

  for (index, &sigma) in sigmas.iter().enumerate() {
    let (pyramid_level, local_sigma) = choose_pyramid_level(sigma, &pyramid, cfg);
    plans.push((pyramid_level, local_sigma));
    grouped[pyramid_level].push((index, local_sigma));
  }

  let mut built: Vec<Option<BlurLevel>> = vec![None; sigmas.len()];

  for (level_idx, items) in grouped.iter_mut().enumerate() {
    items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut current_image = pyramid[level_idx].image.clone();
    let mut current_sigma = 0.0;

    for &(index, target_local_sigma) in items.iter() {
      let delta_sigma = ((target_local_sigma * target_local_sigma)
        - (current_sigma * current_sigma))
        .max(0.0)
        .sqrt();
      if delta_sigma > EPSILON {
        current_image = blur_separable(&current_image, delta_sigma);
        current_sigma = target_local_sigma;
      }

      let (pyramid_level, _) = plans[index];
      built[index] = Some(BlurLevel {
        pyramid_level,
        image: current_image.clone(),
      });
    }
  }

  built.into_iter().map(Option::unwrap).collect()
}

fn build_pyramid(src: &LinearRgbaImage, cfg: &PyramidConfig) -> Vec<PyramidLevel> {
  let mut levels = vec![PyramidLevel {
    image: src.clone(),
    base_sigma2_global: 0.0,
  }];

  while levels.len() < cfg.max_levels {
    let current = levels.last().expect("pyramid has at least one level");
    if current.image.width <= 1 || current.image.height <= 1 {
      break;
    }

    let level_index = levels.len() - 1;
    let scale = (1usize << level_index) as f32;
    let stage_sigma_global = cfg.downsample_stage_sigma * scale;
    levels.push(PyramidLevel {
      image: downsample_half_box(&current.image),
      base_sigma2_global: current.base_sigma2_global + stage_sigma_global * stage_sigma_global,
    });
  }

  levels
}

fn choose_pyramid_level(
  target_sigma_global: f32,
  pyramid: &[PyramidLevel],
  cfg: &PyramidConfig,
) -> (usize, f32) {
  if target_sigma_global <= EPSILON {
    return (0, 0.0);
  }

  let target_sigma2 = target_sigma_global * target_sigma_global;
  let mut best_in_range: Option<(usize, f32, f32)> = None;
  let mut best_any: Option<(usize, f32, f32)> = None;

  for (level_idx, level) in pyramid.iter().enumerate() {
    if level.base_sigma2_global > target_sigma2 + EPSILON {
      continue;
    }

    let scale = (1usize << level_idx) as f32;
    let local_sigma = ((target_sigma2 - level.base_sigma2_global).max(0.0)).sqrt() / scale;
    let score = (local_sigma - cfg.target_local_sigma).abs();
    let candidate = (level_idx, local_sigma, score);
    let in_range = local_sigma <= EPSILON
      || (local_sigma >= cfg.min_local_sigma && local_sigma <= cfg.max_local_sigma);

    if in_range && should_replace(best_in_range, candidate) {
      best_in_range = Some(candidate);
    }
    if should_replace(best_any, candidate) {
      best_any = Some(candidate);
    }
  }

  let (level_idx, local_sigma, _) =
    best_in_range
      .or(best_any)
      .unwrap_or((0, target_sigma_global, 0.0));
  (level_idx, local_sigma)
}

fn should_replace(current: Option<(usize, f32, f32)>, candidate: (usize, f32, f32)) -> bool {
  match current {
    None => true,
    Some(existing) => {
      candidate.2 < existing.2 - EPSILON
        || ((candidate.2 - existing.2).abs() < EPSILON && candidate.0 > existing.0)
    }
  }
}
