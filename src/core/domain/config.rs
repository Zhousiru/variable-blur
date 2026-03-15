use std::{
  error::Error,
  fmt::{self, Display, Formatter},
};

use super::{BlurCurve, SigmaSchedule};

const DEFAULT_CURVE_GAMMA: f32 = 1.6;
const DEFAULT_SCHEDULE_GAMMA: f32 = 2.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityPreset {
  Fast,
  Balanced,
  High,
}

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
  pub schedule: SigmaSchedule,
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

impl Default for VariableBlurConfig {
  fn default() -> Self {
    Self::from_quality(QualityPreset::Balanced)
  }
}

impl QualityPreset {
  pub fn label(self) -> &'static str {
    match self {
      Self::Fast => "Performance",
      Self::Balanced => "Balanced",
      Self::High => "Quality",
    }
  }

  pub fn from_name(value: &str) -> Option<Self> {
    if value.eq_ignore_ascii_case("fast") || value.eq_ignore_ascii_case("performance") {
      Some(Self::Fast)
    } else if value.eq_ignore_ascii_case("balanced") {
      Some(Self::Balanced)
    } else if value.eq_ignore_ascii_case("high") || value.eq_ignore_ascii_case("quality") {
      Some(Self::High)
    } else {
      None
    }
  }

  pub fn default_max_sigma(self) -> f32 {
    match self {
      Self::Fast => 24.0,
      Self::Balanced => 32.0,
      Self::High => 40.0,
    }
  }
}

impl VariableBlurConfig {
  pub fn from_quality(preset: QualityPreset) -> Self {
    let advanced = preset_advanced_settings(preset);
    Self {
      max_sigma: preset.default_max_sigma(),
      steps: advanced.steps,
      curve: default_curve(),
      schedule: default_schedule(),
      pyramid: advanced.pyramid_config(),
    }
  }

  pub fn from_auto_preset(preset: QualityPreset, dimensions: (u32, u32), max_sigma: f32) -> Self {
    let advanced = auto_advanced_settings(preset, dimensions, max_sigma.max(0.01));
    Self {
      max_sigma: max_sigma.max(0.01),
      steps: advanced.steps,
      curve: default_curve(),
      schedule: default_schedule(),
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

pub fn preset_advanced_settings(preset: QualityPreset) -> AdvancedSettings {
  match preset {
    QualityPreset::Fast => AdvancedSettings {
      steps: 7,
      max_levels: 6,
      target_local_sigma: 1.6,
      min_local_sigma: 0.3,
      max_local_sigma: 3.0,
      downsample_stage_sigma: 0.5,
    },
    QualityPreset::Balanced => AdvancedSettings {
      steps: 10,
      max_levels: 4,
      target_local_sigma: 2.0,
      min_local_sigma: 0.5,
      max_local_sigma: 4.0,
      downsample_stage_sigma: 0.5,
    },
    QualityPreset::High => AdvancedSettings {
      steps: 14,
      max_levels: 2,
      target_local_sigma: 2.4,
      min_local_sigma: 0.8,
      max_local_sigma: 5.0,
      downsample_stage_sigma: 0.5,
    },
  }
}

pub fn auto_advanced_settings(
  preset: QualityPreset,
  dimensions: (u32, u32),
  max_sigma: f32,
) -> AdvancedSettings {
  let (base_steps, base_target, min_ratio, max_ratio, depth_bias, level_cap) = match preset {
    QualityPreset::Fast => (7usize, 1.6f32, 0.1875f32, 1.875f32, 1isize, 8usize),
    QualityPreset::Balanced => (10usize, 2.0f32, 0.25f32, 2.0f32, 0isize, 6usize),
    QualityPreset::High => (
      14usize,
      2.4f32,
      1.0f32 / 3.0f32,
      2.0833333f32,
      -2isize,
      4usize,
    ),
  };

  let max_sigma = max_sigma.max(0.01);
  let target_local_sigma = base_target.min(max_sigma.max(0.5));
  let min_local_sigma = (target_local_sigma * min_ratio).max(0.1);
  let max_local_sigma = (target_local_sigma * max_ratio).max(target_local_sigma);

  let min_dimension = dimensions.0.min(dimensions.1).max(1) as f32;
  let size_cap = if min_dimension <= 16.0 {
    1
  } else {
    ((min_dimension / 16.0).log2().floor().max(0.0) as usize) + 1
  };
  let blur_depth = ((max_sigma / target_local_sigma.max(0.01)).max(1.0))
    .log2()
    .ceil() as usize;
  let biased_depth = (blur_depth as isize + depth_bias).max(1) as usize;
  let max_levels = biased_depth.min(size_cap).min(level_cap).max(1);

  let extra_steps = ((max_sigma / 32.0).max(1.0)).log2().ceil() as usize;
  let steps = (base_steps + extra_steps).clamp(2, 24);

  AdvancedSettings {
    steps,
    max_levels,
    target_local_sigma,
    min_local_sigma,
    max_local_sigma,
    downsample_stage_sigma: 0.5,
  }
}

fn default_curve() -> BlurCurve {
  BlurCurve::Power(DEFAULT_CURVE_GAMMA)
}

fn default_schedule() -> SigmaSchedule {
  SigmaSchedule::Power {
    gamma: DEFAULT_SCHEDULE_GAMMA,
  }
}

#[cfg(test)]
mod tests {
  use super::{ConfigValidationError, QualityPreset, VariableBlurConfig};

  #[test]
  fn default_config_matches_balanced_preset() {
    assert_eq!(
      VariableBlurConfig::default(),
      VariableBlurConfig::from_quality(QualityPreset::Balanced),
    );
  }

  #[test]
  fn invalid_local_sigma_range_is_rejected() {
    let mut config = VariableBlurConfig::default();
    config.pyramid.min_local_sigma = 3.0;
    config.pyramid.max_local_sigma = 2.0;
    assert_eq!(
      config.validate(),
      Err(ConfigValidationError::MinLocalSigmaAboveMaxLocalSigma),
    );
  }
}
