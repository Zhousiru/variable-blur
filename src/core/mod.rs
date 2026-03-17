pub mod domain;
mod engine;
mod io;
mod service;

pub use domain::{
  auto_advanced_settings, default_directional_options, normalize_direction,
  preset_advanced_settings, projection_bounds, AdvancedSettings, BlurCurve, ConfigValidationError,
  DirectionalBlurOptions, PyramidConfig, QualityPreset, SigmaSchedule, VariableBlurConfig,
};
pub use io::codec::{encode_dynamic_image, RawImageError};
pub use service::{apply_directional_variable_blur, apply_directional_variable_blur_raw};

pub(crate) const EPSILON: f32 = 1e-4;
