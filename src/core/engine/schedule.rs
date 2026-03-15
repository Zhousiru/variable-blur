use crate::core::{domain::SigmaSchedule, EPSILON};

pub(crate) fn generate_sigmas(schedule: &SigmaSchedule, steps: usize, max_sigma: f32) -> Vec<f32> {
  let steps = steps.max(2);
  let last = (steps - 1) as f32;
  let mut sigmas = Vec::with_capacity(steps);

  for i in 0..steps {
    let t = i as f32 / last;
    let sigma = schedule.eval(t) * max_sigma;
    sigmas.push(sigma.clamp(0.0, max_sigma));
  }

  sigmas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  sigmas.dedup_by(|a, b| (*a - *b).abs() < EPSILON);

  if sigmas.first().copied().unwrap_or(0.0) > EPSILON {
    sigmas.insert(0, 0.0);
  }
  if (sigmas.last().copied().unwrap_or(0.0) - max_sigma).abs() > EPSILON {
    sigmas.push(max_sigma);
  }

  sigmas
}

#[cfg(test)]
mod tests {
  use crate::core::domain::SigmaSchedule;

  use super::generate_sigmas;

  #[test]
  fn generated_sigmas_include_zero_and_max() {
    let sigmas = generate_sigmas(&SigmaSchedule::Power { gamma: 2.8 }, 7, 32.0);
    assert_eq!(sigmas.first().copied(), Some(0.0));
    assert_eq!(sigmas.last().copied(), Some(32.0));
  }
}
