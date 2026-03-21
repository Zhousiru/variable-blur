#[derive(Clone, Debug, PartialEq)]
pub enum BlurCurve {
  Linear,
  Power(f32),
  CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
}

impl BlurCurve {
  pub fn eval(&self, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match *self {
      Self::Linear => t,
      Self::Power(gamma) => t.powf(gamma.max(0.01)),
      Self::CubicBezier { x1, y1, x2, y2 } => eval_cubic_bezier(
        t,
        x1.clamp(0.0, 1.0),
        y1.clamp(0.0, 1.0),
        x2.clamp(0.0, 1.0),
        y2.clamp(0.0, 1.0),
      ),
    }
  }
}

fn eval_cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
  if t <= 0.0 {
    return 0.0;
  }
  if t >= 1.0 {
    return 1.0;
  }

  let mut u = t;
  for _ in 0..5 {
    let x = cubic_bezier_point(u, x1, x2) - t;
    let derivative = cubic_bezier_derivative(u, x1, x2);
    if derivative.abs() < 1e-6 {
      break;
    }
    u = (u - x / derivative).clamp(0.0, 1.0);
  }

  let mut low = 0.0;
  let mut high = 1.0;
  for _ in 0..8 {
    let x = cubic_bezier_point(u, x1, x2);
    if (x - t).abs() < 1e-5 {
      break;
    }
    if x < t {
      low = u;
    } else {
      high = u;
    }
    u = 0.5 * (low + high);
  }

  cubic_bezier_point(u, y1, y2).clamp(0.0, 1.0)
}

fn cubic_bezier_point(t: f32, p1: f32, p2: f32) -> f32 {
  let omt = 1.0 - t;
  3.0 * omt * omt * t * p1 + 3.0 * omt * t * t * p2 + t * t * t
}

fn cubic_bezier_derivative(t: f32, p1: f32, p2: f32) -> f32 {
  let omt = 1.0 - t;
  3.0 * omt * omt * p1 + 6.0 * omt * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

#[cfg(test)]
mod tests {
  use super::BlurCurve;

  #[test]
  fn linear_curve_is_identity() {
    assert_eq!(BlurCurve::Linear.eval(0.25), 0.25);
  }
}
