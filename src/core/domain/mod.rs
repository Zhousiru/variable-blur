mod approximation;
mod config;
mod curve;
mod direction;

pub(crate) use approximation::curve_sampling_complexity;
pub use approximation::{generate_curve_anchors, generate_sigma_anchors, CurveAnchor};
pub use config::{
  advanced_settings_for_quality, auto_advanced_settings, auto_quality_settings, quality_settings,
  AdvancedSettings, ConfigValidationError, PyramidConfig, VariableBlurConfig,
};
pub(crate) use config::{DEFAULT_CURVE_GAMMA, DEFAULT_CURVE_NAME};
pub use curve::BlurCurve;
pub use direction::{
  active_projection_span, default_directional_options, normalize_direction, projection_bounds,
  DirectionalBlurOptions,
};
