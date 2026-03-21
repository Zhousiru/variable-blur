use image::{DynamicImage, GenericImageView, ImageFormat};
use napi::{
  bindgen_prelude::{Buffer, Error, Result},
  Status,
};
use napi_derive::napi;

use crate::core::{
  active_projection_span, apply_directional_variable_blur, apply_directional_variable_blur_raw,
  default_directional_options, encode_dynamic_image, BlurCurve, DirectionalBlurOptions,
  VariableBlurConfig, DEFAULT_CURVE_GAMMA, DEFAULT_CURVE_NAME,
};

#[derive(Clone, Copy, Default)]
enum AdvancedMode {
  #[default]
  Auto,
  Manual,
}

#[napi(object)]
#[derive(Default)]
pub struct VariableBlurInput {
  pub buffer: Buffer,
  pub options: VariableBlurOptions,
}

#[napi(object)]
#[derive(Default)]
pub struct VariableBlurRawInput {
  pub data: Buffer,
  pub width: u32,
  pub height: u32,
  pub channels: u32,
  pub options: VariableBlurOptions,
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
  pub x: f64,
  pub y: f64,
  pub start: Option<f64>,
  pub end: Option<f64>,
  pub quality: Option<f64>,
  pub max_sigma: f64,
  pub curve: Option<String>,
  pub advanced: Option<VariableBlurAdvancedOptions>,
  pub output_format: Option<String>,
}

#[napi(js_name = "variableBlur")]
pub fn variable_blur(input: VariableBlurInput) -> Result<Buffer> {
  let options = input.options;
  let (decoded, input_format) = decode_input_image(&input.buffer)?;
  let blur_options = build_directional_options(&options, decoded.dimensions())?;
  let cfg = build_config(&options, decoded.dimensions(), blur_options)?;

  let output = apply_directional_variable_blur(&decoded, cfg, blur_options);
  let encoded = encode_dynamic_image(
    &output,
    parse_output_format(options.output_format.as_deref())?,
    input_format,
  )
  .map_err(|err| invalid_arg(format!("encode image failed: {err}")))?;

  Ok(encoded.into())
}

#[napi(js_name = "variableBlurRaw")]
pub fn variable_blur_raw(input: VariableBlurRawInput) -> Result<Buffer> {
  let options = input.options;
  let dimensions = (input.width, input.height);
  let blur_options = build_directional_options(&options, dimensions)?;
  let cfg = build_config(&options, dimensions, blur_options)?;

  let output = apply_directional_variable_blur_raw(
    &input.data,
    input.width,
    input.height,
    input.channels,
    cfg,
    blur_options,
  )
  .map_err(|err| invalid_arg(format!("process raw image failed: {err}")))?;

  Ok(output.into())
}

fn decode_input_image(buffer: &Buffer) -> Result<(DynamicImage, ImageFormat)> {
  let input_bytes = buffer.as_ref();
  let input_format = image::guess_format(input_bytes).unwrap_or(ImageFormat::Png);
  let decoded = image::load_from_memory(input_bytes)
    .map_err(|err| invalid_arg(format!("decode image failed: {err}")))?;

  Ok((decoded, input_format))
}

fn build_directional_options(
  options: &VariableBlurOptions,
  dimensions: (u32, u32),
) -> Result<DirectionalBlurOptions> {
  let x = finite("x", options.x)? as f32;
  let y = finite("y", options.y)? as f32;

  let mut blur_options = default_directional_options(dimensions, [x, y]);
  if let Some(start) = options.start {
    blur_options.start = finite("start", start)? as f32;
  }
  if let Some(end) = options.end {
    blur_options.end = finite("end", end)? as f32;
  }

  Ok(blur_options)
}

fn build_config(
  options: &VariableBlurOptions,
  dimensions: (u32, u32),
  blur_options: DirectionalBlurOptions,
) -> Result<VariableBlurConfig> {
  let quality = parse_quality(options.quality)?.unwrap_or(0.5);
  let curve = parse_curve(options.curve.as_deref())?;
  let max_sigma = positive("maxSigma", options.max_sigma)? as f32;
  let blur_span = active_projection_span(
    dimensions,
    blur_options.direction,
    blur_options.start,
    blur_options.end,
  );

  let advanced = options.advanced.as_ref();
  let mode = parse_advanced_mode(advanced.and_then(|value| value.mode.as_deref()))?;
  let mut cfg = match mode {
    AdvancedMode::Auto => VariableBlurConfig::from_auto_quality(
      quality,
      curve.clone(),
      dimensions,
      max_sigma,
      blur_span,
    ),
    AdvancedMode::Manual => VariableBlurConfig::from_quality(quality, curve, max_sigma, blur_span),
  };

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
    cfg.steps = minimum("advanced.steps", steps, 2)? as usize;
  }
  if let Some(max_levels) = advanced.max_levels {
    cfg.pyramid.max_levels = minimum("advanced.maxLevels", max_levels, 1)? as usize;
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

fn parse_curve(value: Option<&str>) -> Result<BlurCurve> {
  let value = value.unwrap_or(DEFAULT_CURVE_NAME).trim();

  if let Some((name, args)) = parse_function_call(value) {
    if name.eq_ignore_ascii_case("power") {
      let [gamma] = parse_fixed_f32_args::<1>("curve", value, args)?;
      return Ok(BlurCurve::Power(positive_f32("curve", gamma, value)?));
    }
    if name.eq_ignore_ascii_case("cubicbezier") || name.eq_ignore_ascii_case("cubic-bezier") {
      let [x1, y1, x2, y2] = parse_fixed_f32_args::<4>("curve", value, args)?;
      return Ok(BlurCurve::CubicBezier { x1, y1, x2, y2 });
    }
  }

  match value {
    value if value.eq_ignore_ascii_case("linear") => Ok(BlurCurve::Linear),
    value if value.eq_ignore_ascii_case("power") => Ok(BlurCurve::Power(DEFAULT_CURVE_GAMMA)),
    value => Err(invalid_arg(format!("unsupported curve: {value}"))),
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

fn parse_quality(value: Option<f64>) -> Result<Option<f32>> {
  match value {
    Some(quality) if quality.is_finite() && (0.0..=1.0).contains(&quality) => {
      Ok(Some(quality as f32))
    }
    Some(_) => Err(invalid_arg("quality must be a finite number within [0, 1]")),
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

fn parse_fixed_f32_args<const N: usize>(
  kind: &str,
  full_value: &str,
  args: &str,
) -> Result<[f32; N]> {
  let values = args
    .split(',')
    .map(|part| {
      let value = part
        .trim()
        .parse::<f32>()
        .map_err(|_| invalid_arg(format!("invalid {kind} spec: {full_value}")))?;
      if value.is_finite() {
        Ok(value)
      } else {
        Err(invalid_arg(format!("invalid {kind} spec: {full_value}")))
      }
    })
    .collect::<Result<Vec<_>>>()?;

  values
    .try_into()
    .map_err(|_| invalid_arg(format!("invalid {kind} spec: {full_value}")))
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

fn positive_f32(kind: &str, value: f32, full_value: &str) -> Result<f32> {
  if value.is_finite() && value > 0.0 {
    Ok(value)
  } else {
    Err(invalid_arg(format!("invalid {kind} spec: {full_value}")))
  }
}

fn positive(name: &str, value: f64) -> Result<f64> {
  if value.is_finite() && value > 0.0 {
    Ok(value)
  } else {
    Err(invalid_arg(format!("{name} must be > 0")))
  }
}

fn minimum(name: &str, value: u32, min: u32) -> Result<u32> {
  if value >= min {
    Ok(value)
  } else {
    Err(invalid_arg(format!("{name} must be >= {min}")))
  }
}

fn finite(name: &str, value: f64) -> Result<f64> {
  if value.is_finite() {
    Ok(value)
  } else {
    Err(invalid_arg(format!("{name} must be a finite number")))
  }
}

fn invalid_arg(reason: impl Into<String>) -> Error {
  Error::new(Status::InvalidArg, reason.into())
}
