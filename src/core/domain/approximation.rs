use crate::core::EPSILON;

use super::BlurCurve;

const CURVE_PROBE_SAMPLES: usize = 256;
const SLOPE_DENSITY_EXPONENT: f32 = 0.5;
const CURVATURE_DENSITY_EXPONENT: f32 = 1.0 / 3.0;
const CURVATURE_DENSITY_WEIGHT: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveAnchor {
  pub t: f32,
  pub sigma: f32,
}

pub fn generate_curve_anchors(curve: &BlurCurve, steps: usize, max_sigma: f32) -> Vec<CurveAnchor> {
  let steps = steps.max(2);
  let max_sigma = max_sigma.max(0.0);
  let profile = CurveSamplingProfile::new(curve, max_sigma);
  let last = (steps - 1) as f32;
  let mut anchors = Vec::with_capacity(steps);

  for index in 0..steps {
    let q = index as f32 / last;
    let t = profile.t_at_quantile(q);
    anchors.push(CurveAnchor {
      t,
      sigma: (curve.eval(t) * max_sigma).clamp(0.0, max_sigma),
    });
  }

  anchors[0] = CurveAnchor { t: 0.0, sigma: 0.0 };
  anchors[steps - 1] = CurveAnchor {
    t: 1.0,
    sigma: max_sigma,
  };
  anchors
}

pub fn generate_sigma_anchors(curve: &BlurCurve, steps: usize, max_sigma: f32) -> Vec<f32> {
  let mut sigmas = generate_curve_anchors(curve, steps, max_sigma)
    .into_iter()
    .map(|anchor| anchor.sigma)
    .collect::<Vec<_>>();

  sigmas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  sigmas.dedup_by(|a, b| (*a - *b).abs() < EPSILON);

  if sigmas.first().copied().unwrap_or(0.0) > EPSILON {
    sigmas.insert(0, 0.0);
  }
  if (sigmas.last().copied().unwrap_or(0.0) - max_sigma.max(0.0)).abs() > EPSILON {
    sigmas.push(max_sigma.max(0.0));
  }

  sigmas
}

pub(crate) fn curve_sampling_complexity(curve: &BlurCurve, max_sigma: f32) -> f32 {
  CurveSamplingProfile::new(curve, max_sigma).total_density
}

struct CurveSamplingProfile {
  ts: Vec<f32>,
  cumulative_density: Vec<f32>,
  total_density: f32,
}

impl CurveSamplingProfile {
  fn new(curve: &BlurCurve, max_sigma: f32) -> Self {
    let sample_count = CURVE_PROBE_SAMPLES.max(2);
    let dt = 1.0 / sample_count as f32;
    let ts = (0..=sample_count)
      .map(|index| index as f32 * dt)
      .collect::<Vec<_>>();
    let sigmas = ts
      .iter()
      .map(|&t| curve.eval(t) * max_sigma.max(0.0))
      .collect::<Vec<_>>();
    let mut density = vec![0.0; ts.len()];

    for (index, slot) in density.iter_mut().enumerate().take(sample_count + 1) {
      let first_derivative = first_derivative(&sigmas, index, dt);
      let second_derivative = second_derivative(&sigmas, index, dt);
      *slot = first_derivative.abs().powf(SLOPE_DENSITY_EXPONENT)
        + CURVATURE_DENSITY_WEIGHT * second_derivative.abs().powf(CURVATURE_DENSITY_EXPONENT);
    }

    let mut cumulative_density = vec![0.0; ts.len()];
    for index in 0..sample_count {
      cumulative_density[index + 1] =
        cumulative_density[index] + 0.5 * (density[index] + density[index + 1]) * dt;
    }

    Self {
      ts,
      total_density: cumulative_density[sample_count],
      cumulative_density,
    }
  }

  fn t_at_quantile(&self, q: f32) -> f32 {
    let q = q.clamp(0.0, 1.0);
    if self.total_density <= EPSILON {
      return q;
    }

    let target = self.total_density * q;
    let hi = self
      .cumulative_density
      .partition_point(|value| *value < target)
      .min(self.cumulative_density.len().saturating_sub(1));
    let lo = hi.saturating_sub(1);

    if hi == lo {
      return self.ts[lo];
    }

    let low_density = self.cumulative_density[lo];
    let high_density = self.cumulative_density[hi];
    if (high_density - low_density).abs() <= EPSILON {
      return self.ts[lo];
    }

    let alpha = ((target - low_density) / (high_density - low_density)).clamp(0.0, 1.0);
    self.ts[lo] + (self.ts[hi] - self.ts[lo]) * alpha
  }
}

fn first_derivative(samples: &[f32], index: usize, dt: f32) -> f32 {
  if index == 0 {
    (samples[1] - samples[0]) / dt
  } else if index + 1 >= samples.len() {
    (samples[index] - samples[index - 1]) / dt
  } else {
    (samples[index + 1] - samples[index - 1]) / (2.0 * dt)
  }
}

fn second_derivative(samples: &[f32], index: usize, dt: f32) -> f32 {
  if index == 0 {
    (samples[2] - 2.0 * samples[1] + samples[0]) / (dt * dt)
  } else if index + 1 >= samples.len() {
    let last = samples.len() - 1;
    (samples[last] - 2.0 * samples[last - 1] + samples[last - 2]) / (dt * dt)
  } else {
    (samples[index + 1] - 2.0 * samples[index] + samples[index - 1]) / (dt * dt)
  }
}

#[cfg(test)]
mod tests {
  use super::{curve_sampling_complexity, generate_curve_anchors, generate_sigma_anchors};
  use crate::core::domain::BlurCurve;

  #[test]
  fn linear_curve_has_positive_sampling_complexity() {
    assert!(curve_sampling_complexity(&BlurCurve::Linear, 32.0) > 0.0);
  }

  #[test]
  fn linear_curve_anchors_are_uniform() {
    let anchors = generate_curve_anchors(&BlurCurve::Linear, 5, 16.0);
    let ts = anchors.iter().map(|anchor| anchor.t).collect::<Vec<_>>();
    assert_eq!(ts, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
  }

  #[test]
  fn convex_curve_biases_anchors_towards_the_end() {
    let anchors = generate_curve_anchors(&BlurCurve::Power(2.8), 7, 32.0);
    let gaps = anchors
      .windows(2)
      .map(|pair| pair[1].t - pair[0].t)
      .collect::<Vec<_>>();
    assert!(gaps.last().copied().unwrap_or(0.0) < gaps.first().copied().unwrap_or(0.0));
  }

  #[test]
  fn concave_curve_biases_anchors_towards_the_start() {
    let anchors = generate_curve_anchors(&BlurCurve::Power(0.35), 7, 32.0);
    let gaps = anchors
      .windows(2)
      .map(|pair| pair[1].t - pair[0].t)
      .collect::<Vec<_>>();
    assert!(gaps.first().copied().unwrap_or(0.0) < gaps.last().copied().unwrap_or(0.0));
  }

  #[test]
  fn sigma_anchors_are_sorted_and_bounded() {
    let sigmas = generate_sigma_anchors(&BlurCurve::Power(0.35), 7, 32.0);
    assert_eq!(sigmas.first().copied(), Some(0.0));
    assert_eq!(sigmas.last().copied(), Some(32.0));
    assert!(sigmas.windows(2).all(|pair| pair[0] <= pair[1]));
  }
}
