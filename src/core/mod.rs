pub mod domain;
mod engine;
mod io;
mod service;

pub use domain::{
  active_projection_span, advanced_settings_for_quality, auto_advanced_settings,
  auto_quality_settings, default_directional_options, generate_curve_anchors,
  generate_sigma_anchors, normalize_direction, projection_bounds, quality_settings,
  AdvancedSettings, BlurCurve, ConfigValidationError, CurveAnchor, DirectionalBlurOptions,
  PyramidConfig, VariableBlurConfig,
};
pub(crate) use domain::{DEFAULT_CURVE_GAMMA, DEFAULT_CURVE_NAME};
pub use io::codec::{encode_dynamic_image, RawImageError};
pub use service::{
  apply_directional_variable_blur, apply_directional_variable_blur_raw,
  generate_directional_step_map,
};

pub(crate) const EPSILON: f32 = 1e-4;
