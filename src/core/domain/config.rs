use std::{
  error::Error,
  fmt::{self, Display, Formatter},
};

use super::{curve_sampling_complexity, BlurCurve};

pub(crate) const DEFAULT_CURVE_NAME: &str = "power";
pub(crate) const DEFAULT_CURVE_GAMMA: f32 = 1.6;
const DEFAULT_DOWNSAMPLE_STAGE_SIGMA: f32 = 0.5;
const MIN_AUTO_LEVEL_EXTENT: f32 = 16.0;
const MIN_LOCAL_SIGMA: f32 = 0.5;
const MAX_LOCAL_SIGMA_CAP: f32 = 8.0;
const MIN_TARGET_LOCAL_SIGMA: f32 = 1.5;
const MAX_TARGET_LOCAL_SIGMA: f32 = 2.5;
const TARGET_LOCAL_SIGMA_WARP: f32 = 0.8;
const LOCAL_SIGMA_RANGE_RATIO: f32 = 2.0;
const MAX_AUTO_LEVELS: usize = 8;
const LOW_QUALITY_COMPLEXITY_BUDGET: f32 = 48.0;
const HIGH_QUALITY_COMPLEXITY_BUDGET: f32 = 16.0;
const MIN_STEPS: usize = 2;
const MAX_STEPS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvancedSettings {
  pub steps: usize,
  pub max_levels: usize,
  pub target_local_sigma: f32,
  pub min_local_sigma: f32,
  pub max_local_sigma: f32,
  pub downsample_stage_sigma: f32,
}

impl AdvancedSettings {
  pub(crate) fn pyramid_config(self) -> PyramidConfig {
    PyramidConfig {
      max_levels: self.max_levels,
      target_local_sigma: self.target_local_sigma,
      min_local_sigma: self.min_local_sigma,
      max_local_sigma: self.max_local_sigma,
      downsample_stage_sigma: self.downsample_stage_sigma,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PyramidConfig {
  pub max_levels: usize,
  pub target_local_sigma: f32,
  pub min_local_sigma: f32,
  pub max_local_sigma: f32,
  pub downsample_stage_sigma: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableBlurConfig {
  pub max_sigma: f32,
  pub steps: usize,
  pub curve: BlurCurve,
  pub pyramid: PyramidConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigValidationError {
  MinLocalSigmaAboveMaxLocalSigma,
  TargetLocalSigmaOutOfRange,
}

impl Display for ConfigValidationError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Self::MinLocalSigmaAboveMaxLocalSigma => {
        f.write_str("advanced.minLocalSigma must be <= advanced.maxLocalSigma")
      }
      Self::TargetLocalSigmaOutOfRange => f.write_str(
        "advanced.targetLocalSigma must be within [advanced.minLocalSigma, advanced.maxLocalSigma]",
      ),
    }
  }
}

impl Error for ConfigValidationError {}

impl VariableBlurConfig {
  pub fn from_quality(quality: f32, curve: BlurCurve, max_sigma: f32, blur_span: f32) -> Self {
    let advanced = quality_settings(quality, &curve, max_sigma, blur_span);
    Self {
      max_sigma: max_sigma.max(0.01),
      steps: advanced.steps,
      curve,
      pyramid: advanced.pyramid_config(),
    }
  }

  pub fn from_auto_quality(
    quality: f32,
    curve: BlurCurve,
    dimensions: (u32, u32),
    max_sigma: f32,
    blur_span: f32,
  ) -> Self {
    let advanced =
      auto_quality_settings(quality, &curve, dimensions, max_sigma.max(0.01), blur_span);
    Self {
      max_sigma: max_sigma.max(0.01),
      steps: advanced.steps,
      curve,
      pyramid: advanced.pyramid_config(),
    }
  }

  pub fn validate(&self) -> Result<(), ConfigValidationError> {
    if self.pyramid.min_local_sigma > self.pyramid.max_local_sigma {
      return Err(ConfigValidationError::MinLocalSigmaAboveMaxLocalSigma);
    }
    if self.pyramid.target_local_sigma < self.pyramid.min_local_sigma
      || self.pyramid.target_local_sigma > self.pyramid.max_local_sigma
    {
      return Err(ConfigValidationError::TargetLocalSigmaOutOfRange);
    }

    Ok(())
  }
}

/// Derive advanced settings from quality `q ∈ [0, 1]` and blur parameters.
///
/// `curve` defines the target sigma field, while `q` controls how aggressively
/// the runtime approximates it using discrete sigma anchors and the blur pyramid.
pub fn quality_settings(
  q: f32,
  curve: &BlurCurve,
  max_sigma: f32,
  blur_span: f32,
) -> AdvancedSettings {
  let q = q.clamp(0.0, 1.0);
  let max_sigma = max_sigma.max(0.01);
  let steps = compute_steps(q, curve, max_sigma, blur_span);
  let target_local_sigma = target_local_sigma_for_quality(q).min(max_sigma);
  let (min_local_sigma, max_local_sigma) = derive_local_sigma_range(target_local_sigma);
  let max_levels =
    levels_preferred_by_quality(q).min(levels_needed_by_sigma(max_sigma, target_local_sigma));

  AdvancedSettings {
    steps,
    max_levels,
    target_local_sigma,
    min_local_sigma,
    max_local_sigma,
    downsample_stage_sigma: DEFAULT_DOWNSAMPLE_STAGE_SIGMA,
  }
}

/// Like [`quality_settings`], but additionally constrains pyramid depth by
/// actual image dimensions.
pub fn auto_quality_settings(
  q: f32,
  curve: &BlurCurve,
  dimensions: (u32, u32),
  max_sigma: f32,
  blur_span: f32,
) -> AdvancedSettings {
  let image_min_side = dimensions.0.min(dimensions.1).max(1) as f32;
  let mut settings = quality_settings(q, curve, max_sigma, blur_span);
  settings.max_levels = settings
    .max_levels
    .min(levels_possible_by_image(image_min_side))
    .max(1);
  settings
}

pub fn advanced_settings_for_quality(
  quality: f32,
  curve: &BlurCurve,
  max_sigma: f32,
  blur_span: f32,
) -> AdvancedSettings {
  quality_settings(quality, curve, max_sigma, blur_span)
}

pub fn auto_advanced_settings(
  quality: f32,
  curve: &BlurCurve,
  dimensions: (u32, u32),
  max_sigma: f32,
  blur_span: f32,
) -> AdvancedSettings {
  auto_quality_settings(quality, curve, dimensions, max_sigma, blur_span)
}

/// Derive an anchor budget from the target sigma field and active blur span.
///
/// Two effects need sampling budget:
/// 1. a linear sigma ramp still needs anchors because runtime blending is only
///    piecewise linear in sigma;
/// 2. curved ramps need extra anchors where the shape bends.
///
/// We encode both in a sampling complexity term derived from the curve, then
/// scale it by `sqrt(active_span)` so short blur regions naturally need fewer
/// anchors. `quality` only controls the allowed complexity budget.
fn compute_steps(q: f32, curve: &BlurCurve, max_sigma: f32, blur_span: f32) -> usize {
  let span_factor = blur_span.abs().max(1.0).sqrt();
  let complexity = curve_sampling_complexity(curve, max_sigma.max(0.01)) * span_factor;
  let budget = geometric_lerp(
    LOW_QUALITY_COMPLEXITY_BUDGET,
    HIGH_QUALITY_COMPLEXITY_BUDGET,
    q,
  );
  let extra_steps = (complexity / budget).round().max(0.0) as usize;
  (MIN_STEPS + extra_steps).clamp(MIN_STEPS, MAX_STEPS)
}

fn geometric_lerp(min: f32, max: f32, t: f32) -> f32 {
  min * (max / min).powf(t)
}

fn target_local_sigma_for_quality(q: f32) -> f32 {
  let q = q.clamp(0.0, 1.0);
  let t = (q - TARGET_LOCAL_SIGMA_WARP * q * (1.0 - q) * (2.0 * q - 1.0)).clamp(0.0, 1.0);
  MIN_TARGET_LOCAL_SIGMA + (MAX_TARGET_LOCAL_SIGMA - MIN_TARGET_LOCAL_SIGMA) * t
}

fn derive_local_sigma_range(target_local_sigma: f32) -> (f32, f32) {
  if target_local_sigma <= MIN_LOCAL_SIGMA {
    return (
      target_local_sigma,
      target_local_sigma.min(MAX_LOCAL_SIGMA_CAP),
    );
  }

  let min_local_sigma = (target_local_sigma / LOCAL_SIGMA_RANGE_RATIO)
    .max(MIN_LOCAL_SIGMA)
    .min(target_local_sigma);
  let max_local_sigma = (target_local_sigma * LOCAL_SIGMA_RANGE_RATIO)
    .min(MAX_LOCAL_SIGMA_CAP)
    .max(target_local_sigma);
  (min_local_sigma, max_local_sigma)
}

fn levels_preferred_by_quality(q: f32) -> usize {
  1 + ((1.0 - q.clamp(0.0, 1.0)) * (MAX_AUTO_LEVELS.saturating_sub(1)) as f32).round() as usize
}

fn levels_needed_by_sigma(max_sigma: f32, target_local_sigma: f32) -> usize {
  1 + log2_ceil_ratio(max_sigma, target_local_sigma.max(0.01))
}

fn levels_possible_by_image(image_min_side: f32) -> usize {
  1 + log2_floor_ratio(image_min_side, MIN_AUTO_LEVEL_EXTENT)
}

fn log2_floor_ratio(value: f32, unit: f32) -> usize {
  if value <= unit {
    0
  } else {
    ((value / unit).log2().floor().max(0.0)) as usize
  }
}

fn log2_ceil_ratio(value: f32, unit: f32) -> usize {
  if value <= unit {
    0
  } else {
    ((value / unit).log2().ceil().max(0.0)) as usize
  }
}

#[cfg(test)]
mod tests {
  use super::{auto_quality_settings, quality_settings, BlurCurve, DEFAULT_CURVE_GAMMA};

  #[test]
  fn higher_quality_uses_fewer_pyramid_levels() {
    let curve = BlurCurve::Power(DEFAULT_CURVE_GAMMA);
    let low = auto_quality_settings(0.0, &curve, (1600, 1200), 32.0, 512.0);
    let high = auto_quality_settings(1.0, &curve, (1600, 1200), 32.0, 512.0);

    assert!(low.max_levels > high.max_levels);
    assert_eq!(high.max_levels, 1);
  }

  #[test]
  fn steps_grow_with_blur_span() {
    let curve = BlurCurve::Power(DEFAULT_CURVE_GAMMA);
    let short = quality_settings(0.5, &curve, 32.0, 64.0);
    let long = quality_settings(0.5, &curve, 32.0, 512.0);

    assert!(long.steps > short.steps);
  }

  #[test]
  fn linear_curve_still_needs_multiple_steps() {
    let linear = quality_settings(0.5, &BlurCurve::Linear, 32.0, 512.0);
    assert!(linear.steps > 2);
  }

  #[test]
  fn short_spans_limit_curve_penalty() {
    let short = quality_settings(0.5, &BlurCurve::Power(DEFAULT_CURVE_GAMMA), 32.0, 32.0);
    let long = quality_settings(0.5, &BlurCurve::Power(DEFAULT_CURVE_GAMMA), 32.0, 512.0);

    assert!(long.steps > short.steps);
  }

  #[test]
  fn higher_quality_uses_more_steps_for_curved_fields() {
    let low = quality_settings(0.0, &BlurCurve::Power(DEFAULT_CURVE_GAMMA), 32.0, 512.0);
    let high = quality_settings(1.0, &BlurCurve::Power(DEFAULT_CURVE_GAMMA), 32.0, 512.0);

    assert!(high.steps > low.steps);
  }
}
