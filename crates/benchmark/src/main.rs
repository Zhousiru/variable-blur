use std::{
  fs,
  hint::black_box,
  io,
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

use clap::Parser;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};
use sysinfo::System;
use variable_blur::core::{
  apply_directional_variable_blur, default_directional_options, DirectionalBlurOptions,
  QualityPreset, VariableBlurConfig,
};

#[derive(Debug, Parser)]
#[command(
  name = "variable_blur_bench",
  about = "Benchmark Variable Blur presets against a single input image."
)]
struct Args {
  #[arg(long, short = 'i', value_name = "PATH")]
  image: PathBuf,
  #[arg(long, default_value_t = 3)]
  warmup: usize,
  #[arg(long, default_value_t = 10)]
  runs: usize,
  #[arg(long, default_value_t = 1.0)]
  direction_x: f32,
  #[arg(long, default_value_t = 0.0)]
  direction_y: f32,
  #[arg(long)]
  start: Option<f32>,
  #[arg(long)]
  end: Option<f32>,
  #[arg(long)]
  max_sigma: Option<f32>,
}

#[derive(Debug)]
struct EnvironmentInfo {
  cpu_name: String,
  physical_cores: Option<usize>,
  logical_cores: usize,
  os_name: Option<String>,
}

#[derive(Debug)]
struct ImageInfo {
  format: Option<ImageFormat>,
  file_size_bytes: u64,
  width: u32,
  height: u32,
  color_type: String,
}

#[derive(Clone, Debug)]
struct BenchmarkSettings {
  warmup: usize,
  runs: usize,
  options: DirectionalBlurOptions,
  max_sigma_override: Option<f32>,
}

#[derive(Clone, Debug)]
struct BenchmarkResult {
  preset: QualityPreset,
  stats: Stats,
}

#[derive(Clone, Debug)]
struct Stats {
  mean_ms: f64,
  median_ms: f64,
  min_ms: f64,
  max_ms: f64,
  p95_ms: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();
  validate_args(&args)?;

  let environment = read_environment();
  let (image, image_info) = load_image(&args.image)?;
  let settings = build_settings(&args, image.dimensions());

  let results = [
    QualityPreset::Fast,
    QualityPreset::Balanced,
    QualityPreset::High,
  ]
  .into_iter()
  .map(|preset| benchmark_preset(&image, preset, &settings))
  .collect::<Result<Vec<_>, _>>()?;

  print_report(&environment, &image_info, &settings, &results);
  Ok(())
}

fn validate_args(args: &Args) -> Result<(), io::Error> {
  if args.runs == 0 {
    return Err(io::Error::other("--runs must be greater than 0"));
  }
  if let Some(max_sigma) = args.max_sigma {
    if !(max_sigma.is_finite() && max_sigma > 0.0) {
      return Err(io::Error::other("--max-sigma must be a finite value > 0"));
    }
  }

  Ok(())
}

fn read_environment() -> EnvironmentInfo {
  let mut system = System::new_all();
  system.refresh_cpu_all();

  let cpu_name = system
    .cpus()
    .first()
    .map(|cpu| cpu.brand().trim().to_owned())
    .filter(|value| !value.is_empty())
    .or_else(|| std::env::var("PROCESSOR_IDENTIFIER").ok())
    .unwrap_or_else(|| "Unknown CPU".to_owned());

  let os_name = System::long_os_version().or_else(System::name);

  EnvironmentInfo {
    cpu_name,
    physical_cores: System::physical_core_count(),
    logical_cores: system.cpus().len(),
    os_name,
  }
}

fn load_image(path: &Path) -> Result<(DynamicImage, ImageInfo), Box<dyn std::error::Error>> {
  let metadata = fs::metadata(path)?;
  let reader = ImageReader::open(path)?.with_guessed_format()?;
  let format = reader.format();
  let image = reader.decode()?;
  let (width, height) = image.dimensions();
  let info = ImageInfo {
    format,
    file_size_bytes: metadata.len(),
    width,
    height,
    color_type: format!("{:?}", image.color()),
  };

  Ok((image, info))
}

fn build_settings(args: &Args, dimensions: (u32, u32)) -> BenchmarkSettings {
  let mut options = default_directional_options(dimensions, [args.direction_x, args.direction_y]);
  if let Some(start) = args.start {
    options.start = start;
  }
  if let Some(end) = args.end {
    options.end = end;
  }

  BenchmarkSettings {
    warmup: args.warmup,
    runs: args.runs,
    options,
    max_sigma_override: args.max_sigma,
  }
}

fn benchmark_preset(
  image: &DynamicImage,
  preset: QualityPreset,
  settings: &BenchmarkSettings,
) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
  let config = VariableBlurConfig::from_auto_preset(
    preset,
    image.dimensions(),
    settings
      .max_sigma_override
      .unwrap_or_else(|| preset.default_max_sigma()),
  );
  config
    .validate()
    .map_err(|err| io::Error::other(err.to_string()))?;

  for _ in 0..settings.warmup {
    let output = apply_directional_variable_blur(image, config.clone(), settings.options);
    black_box(output);
  }

  let mut samples = Vec::with_capacity(settings.runs);
  for _ in 0..settings.runs {
    let started = Instant::now();
    let output = apply_directional_variable_blur(image, config.clone(), settings.options);
    black_box(output);
    samples.push(started.elapsed());
  }

  Ok(BenchmarkResult {
    preset,
    stats: Stats::from_samples(&samples),
  })
}

impl Stats {
  fn from_samples(samples: &[Duration]) -> Self {
    let mut values = samples
      .iter()
      .map(|duration| duration.as_secs_f64() * 1_000.0)
      .collect::<Vec<_>>();
    values.sort_by(|left, right| left.total_cmp(right));

    let total_ms = values.iter().sum::<f64>();
    let mean_ms = total_ms / values.len() as f64;
    let median_ms = percentile(&values, 0.5);
    let p95_ms = percentile(&values, 0.95);
    let min_ms = *values.first().unwrap_or(&0.0);
    let max_ms = *values.last().unwrap_or(&0.0);

    Self {
      mean_ms,
      median_ms,
      min_ms,
      max_ms,
      p95_ms,
    }
  }
}

fn percentile(sorted_values: &[f64], percentile: f64) -> f64 {
  if sorted_values.is_empty() {
    return 0.0;
  }

  let rank = percentile.clamp(0.0, 1.0) * (sorted_values.len().saturating_sub(1) as f64);
  let lower = rank.floor() as usize;
  let upper = rank.ceil() as usize;
  if lower == upper {
    return sorted_values[lower];
  }

  let weight = rank - lower as f64;
  sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
}

fn print_report(
  environment: &EnvironmentInfo,
  image: &ImageInfo,
  settings: &BenchmarkSettings,
  results: &[BenchmarkResult],
) {
  println!(
    "Machine       : {} | {} | {}",
    environment.os_name.as_deref().unwrap_or("Unknown OS"),
    environment.cpu_name,
    environment
      .physical_cores
      .map(|value| format!("{value}C / {}T", environment.logical_cores))
      .unwrap_or_else(|| format!("?C / {}T", environment.logical_cores)),
  );
  println!(
    "Image         : {}x{} | {} | {} | {}",
    image.width,
    image.height,
    image
      .format
      .map(|value| format!("{value:?}"))
      .unwrap_or_else(|| "Unknown".to_owned()),
    image.color_type,
    format_bytes(image.file_size_bytes),
  );
  println!(
    "Benchmark     : {} warmup | {} measured",
    settings.warmup, settings.runs
  );
  println!(
    "Direction     : [{:.4}, {:.4}] | start {:.4} | end {:.4}",
    settings.options.direction[0],
    settings.options.direction[1],
    settings.options.start,
    settings.options.end,
  );
  println!(
    "Sigma override: {}",
    settings
      .max_sigma_override
      .map(|value| format!("{value:.2}"))
      .unwrap_or_else(|| "preset default".to_owned()),
  );
  println!();
  println!(
    "{:<12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
    "Preset", "avg", "median", "p95", "min", "max", "MPix/s"
  );

  let megapixels = (image.width as f64 * image.height as f64) / 1_000_000.0;
  for result in results {
    println!(
      "{:<12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10.2}",
      preset_name(result.preset),
      format_ms(result.stats.mean_ms),
      format_ms(result.stats.median_ms),
      format_ms(result.stats.p95_ms),
      format_ms(result.stats.min_ms),
      format_ms(result.stats.max_ms),
      throughput_mpix_per_sec(megapixels, result.stats.mean_ms),
    );
  }
}

fn preset_name(preset: QualityPreset) -> &'static str {
  match preset {
    QualityPreset::Fast => "Fast",
    QualityPreset::Balanced => "Balanced",
    QualityPreset::High => "High",
  }
}

fn format_ms(value: f64) -> String {
  format!("{value:.2} ms")
}

fn throughput_mpix_per_sec(megapixels: f64, mean_ms: f64) -> f64 {
  if mean_ms <= f64::EPSILON {
    0.0
  } else {
    megapixels / (mean_ms / 1_000.0)
  }
}

fn format_bytes(value: u64) -> String {
  const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

  let mut size = value as f64;
  let mut unit_index = 0usize;
  while size >= 1024.0 && unit_index + 1 < UNITS.len() {
    size /= 1024.0;
    unit_index += 1;
  }

  if unit_index == 0 {
    format!("{value} {}", UNITS[unit_index])
  } else {
    format!("{size:.2} {}", UNITS[unit_index])
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::{percentile, Stats};

  #[test]
  fn percentile_interpolates_between_neighbors() {
    let values = [10.0, 20.0, 30.0, 40.0];
    assert_eq!(percentile(&values, 0.5), 25.0);
    assert_eq!(percentile(&values, 0.95), 38.5);
  }

  #[test]
  fn stats_are_computed_from_ms_values() {
    let stats = Stats::from_samples(&[
      Duration::from_millis(10),
      Duration::from_millis(20),
      Duration::from_millis(30),
    ]);

    assert_eq!(stats.mean_ms, 20.0);
    assert_eq!(stats.median_ms, 20.0);
    assert_eq!(stats.min_ms, 10.0);
    assert_eq!(stats.max_ms, 30.0);
  }
}
