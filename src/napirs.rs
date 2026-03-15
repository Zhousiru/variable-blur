use image::{GenericImageView, ImageFormat};
use napi::{
  bindgen_prelude::{Buffer, Error, Result},
  Status,
};
use napi_derive::napi;

use crate::core::{
  apply_directional_variable_blur, default_directional_options, encode_dynamic_image, BlurCurve,
  QualityPreset, SigmaSchedule, VariableBlurConfig,
};

#[derive(Default)]
enum AdvancedMode {
  #[default]
  Auto,
  Manual,
}

#[napi(object)]
#[derive(Default)]
pub struct VariableBlurInput {
  pub buffer: Buffer,
  pub options: Option<VariableBlurOptions>,
}

#[napi(object)]
#[derive(Default)]
pub struct VariableBlurAdvancedOptions {
  pub mode: Option<String>,
  pub steps: Option<u32>,
  pub max_levels: Option<u32>,
  pub target_local_sigma: Option<f64>,
  pub min_local_sigma: Option<f64>,
  pub max_local_sigma: Option<f64>,
  pub downsample_stage_sigma: Option<f64>,
}

#[napi(object)]
#[derive(Default)]
pub struct VariableBlurOptions {
  pub x: Option<f64>,
  pub y: Option<f64>,
  pub start: Option<f64>,
  pub end: Option<f64>,
  pub preset: Option<String>,
  pub max_sigma: Option<f64>,
  pub curve: Option<String>,
  pub schedule: Option<String>,
  pub advanced: Option<VariableBlurAdvancedOptions>,
  pub output_format: Option<String>,
}

#[napi(js_name = "variableBlur")]
pub fn variable_blur(input: VariableBlurInput) -> Result<Buffer> {
  let options = input.options.unwrap_or_default();
  let input_bytes = input.buffer.to_vec();
  let input_format = image::guess_format(&input_bytes).unwrap_or(ImageFormat::Png);
  let decoded = image::load_from_memory(&input_bytes)
    .map_err(|err| invalid_arg(format!("decode image failed: {err}")))?;
  let cfg = build_config(&options, decoded.dimensions())?;

  let base_direction = [
    options.x.unwrap_or(1.0) as f32,
    options.y.unwrap_or(0.0) as f32,
  ];
  let mut blur_options = default_directional_options(decoded.dimensions(), base_direction);
  if let Some(start) = options.start {
    blur_options.start = start as f32;
  }
  if let Some(end) = options.end {
    blur_options.end = end as f32;
  }

  let output = apply_directional_variable_blur(&decoded, cfg, blur_options);
  let encoded = encode_dynamic_image(
    &output,
    parse_output_format(options.output_format.as_deref())?,
    input_format,
  )
  .map_err(|err| invalid_arg(format!("encode image failed: {err}")))?;

  Ok(encoded.into())
}

fn build_config(
  options: &VariableBlurOptions,
  dimensions: (u32, u32),
) -> Result<VariableBlurConfig> {
  let preset = parse_preset(options.preset.as_deref())?.unwrap_or(QualityPreset::Balanced);
  let max_sigma = options
    .max_sigma
    .map(|value| positive("maxSigma", value).map(|result| result as f32))
    .transpose()?
    .unwrap_or_else(|| preset.default_max_sigma());

  let advanced = options.advanced.as_ref();
  let mode = parse_advanced_mode(advanced.and_then(|value| value.mode.as_deref()))?;
  let mut cfg = match mode {
    AdvancedMode::Auto => VariableBlurConfig::from_auto_preset(preset, dimensions, max_sigma),
    AdvancedMode::Manual => {
      let mut cfg = VariableBlurConfig::from_quality(preset);
      cfg.max_sigma = max_sigma;
      cfg
    }
  };

  cfg.curve = parse_curve(options)?;
  cfg.schedule = parse_schedule(options)?;

  if let Some(advanced) = advanced {
    apply_advanced_overrides(&mut cfg, advanced)?;
  }

  cfg.validate().map_err(|err| invalid_arg(err.to_string()))?;
  Ok(cfg)
}

fn apply_advanced_overrides(
  cfg: &mut VariableBlurConfig,
  advanced: &VariableBlurAdvancedOptions,
) -> Result<()> {
  if let Some(steps) = advanced.steps {
    cfg.steps = steps.max(2) as usize;
  }
  if let Some(max_levels) = advanced.max_levels {
    cfg.pyramid.max_levels = max_levels.max(1) as usize;
  }
  if let Some(target_local_sigma) = advanced.target_local_sigma {
    cfg.pyramid.target_local_sigma =
      positive("advanced.targetLocalSigma", target_local_sigma)? as f32;
  }
  if let Some(min_local_sigma) = advanced.min_local_sigma {
    cfg.pyramid.min_local_sigma = positive("advanced.minLocalSigma", min_local_sigma)? as f32;
  }
  if let Some(max_local_sigma) = advanced.max_local_sigma {
    cfg.pyramid.max_local_sigma = positive("advanced.maxLocalSigma", max_local_sigma)? as f32;
  }
  if let Some(downsample_stage_sigma) = advanced.downsample_stage_sigma {
    cfg.pyramid.downsample_stage_sigma =
      positive("advanced.downsampleStageSigma", downsample_stage_sigma)? as f32;
  }

  Ok(())
}

fn parse_curve(options: &VariableBlurOptions) -> Result<BlurCurve> {
  let value = options.curve.as_deref().unwrap_or("power").trim();

  if let Some((name, args)) = parse_function_call(value) {
    if name.eq_ignore_ascii_case("power") {
      return Ok(BlurCurve::Power(
        parse_exact_f32_args("curve", value, args, 1)?[0].max(0.01),
      ));
    }
    if name.eq_ignore_ascii_case("cubicbezier") || name.eq_ignore_ascii_case("cubic-bezier") {
      let values = parse_exact_f32_args("curve", value, args, 4)?;
      return Ok(BlurCurve::CubicBezier {
        x1: values[0],
        y1: values[1],
        x2: values[2],
        y2: values[3],
      });
    }
  }

  match value {
    value if value.eq_ignore_ascii_case("linear") => Ok(BlurCurve::Linear),
    value if value.eq_ignore_ascii_case("power") => Ok(BlurCurve::Power(1.6)),
    value => Err(invalid_arg(format!("unsupported curve: {value}"))),
  }
}

fn parse_schedule(options: &VariableBlurOptions) -> Result<SigmaSchedule> {
  let value = options.schedule.as_deref().unwrap_or("power").trim();

  if let Some((name, args)) = parse_function_call(value) {
    if name.eq_ignore_ascii_case("power") {
      return Ok(SigmaSchedule::Power {
        gamma: parse_exact_f32_args("schedule", value, args, 1)?[0].max(0.01),
      });
    }
  }

  match value {
    value if value.eq_ignore_ascii_case("linear") => Ok(SigmaSchedule::Linear),
    value if value.eq_ignore_ascii_case("power") => Ok(SigmaSchedule::Power { gamma: 2.8 }),
    value => Err(invalid_arg(format!("unsupported schedule: {value}"))),
  }
}

fn parse_advanced_mode(value: Option<&str>) -> Result<AdvancedMode> {
  match value {
    Some(name) if name.eq_ignore_ascii_case("manual") => Ok(AdvancedMode::Manual),
    Some(name) if name.eq_ignore_ascii_case("auto") => Ok(AdvancedMode::Auto),
    Some(name) => Err(invalid_arg(format!("unsupported advanced mode: {name}"))),
    None => Ok(AdvancedMode::Auto),
  }
}

fn parse_preset(value: Option<&str>) -> Result<Option<QualityPreset>> {
  match value {
    Some(name) => QualityPreset::from_name(name)
      .map(Some)
      .ok_or_else(|| invalid_arg(format!("unsupported preset: {name}"))),
    None => Ok(None),
  }
}

fn parse_function_call(value: &str) -> Option<(&str, &str)> {
  let trimmed = value.trim();
  let open_index = trimmed.find('(')?;
  if !trimmed.ends_with(')') {
    return None;
  }

  let name = trimmed[..open_index].trim();
  let args = trimmed[(open_index + 1)..(trimmed.len() - 1)].trim();
  if name.is_empty() || args.is_empty() {
    return None;
  }

  Some((name, args))
}

fn parse_exact_f32_args(
  kind: &str,
  full_value: &str,
  args: &str,
  expected: usize,
) -> Result<Vec<f32>> {
  let values = args
    .split(',')
    .map(|part| {
      part
        .trim()
        .parse::<f32>()
        .map_err(|_| invalid_arg(format!("invalid {kind} spec: {full_value}")))
    })
    .collect::<Result<Vec<_>>>()?;

  if values.len() != expected {
    return Err(invalid_arg(format!("invalid {kind} spec: {full_value}")));
  }

  Ok(values)
}

fn parse_output_format(value: Option<&str>) -> Result<Option<ImageFormat>> {
  match value {
    Some(format) if format.eq_ignore_ascii_case("png") => Ok(Some(ImageFormat::Png)),
    Some(format) if format.eq_ignore_ascii_case("jpeg") || format.eq_ignore_ascii_case("jpg") => {
      Ok(Some(ImageFormat::Jpeg))
    }
    Some(format) if format.eq_ignore_ascii_case("webp") => Ok(Some(ImageFormat::WebP)),
    Some(format) if format.eq_ignore_ascii_case("bmp") => Ok(Some(ImageFormat::Bmp)),
    Some(format) if format.eq_ignore_ascii_case("tiff") => Ok(Some(ImageFormat::Tiff)),
    Some(format) if format.eq_ignore_ascii_case("tga") => Ok(Some(ImageFormat::Tga)),
    Some(format) => Err(invalid_arg(format!("unsupported output format: {format}"))),
    None => Ok(None),
  }
}

fn positive(name: &str, value: f64) -> Result<f64> {
  if value.is_finite() && value > 0.0 {
    Ok(value)
  } else {
    Err(invalid_arg(format!("{name} must be > 0")))
  }
}

fn invalid_arg(reason: String) -> Error {
  Error::new(Status::InvalidArg, reason)
}
