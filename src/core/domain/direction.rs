use crate::core::EPSILON;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalBlurOptions {
  pub direction: [f32; 2],
  pub start: f32,
  pub end: f32,
}

pub fn default_directional_options(
  dimensions: (u32, u32),
  direction: [f32; 2],
) -> DirectionalBlurOptions {
  let direction = normalize_direction(direction);
  let (start, end) = projection_bounds(dimensions, direction);
  DirectionalBlurOptions {
    direction,
    start,
    end,
  }
}

pub fn normalize_direction(direction: [f32; 2]) -> [f32; 2] {
  let length = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
  if length <= EPSILON {
    [1.0, 0.0]
  } else {
    [direction[0] / length, direction[1] / length]
  }
}

pub fn projection_bounds(dimensions: (u32, u32), direction: [f32; 2]) -> (f32, f32) {
  let (width, height) = dimensions;
  let corners = [
    [0.0, 0.0],
    [width as f32, 0.0],
    [0.0, height as f32],
    [width as f32, height as f32],
  ];
  let mut min_projection = f32::INFINITY;
  let mut max_projection = f32::NEG_INFINITY;

  for corner in corners {
    let projection = corner[0] * direction[0] + corner[1] * direction[1];
    min_projection = min_projection.min(projection);
    max_projection = max_projection.max(projection);
  }

  (min_projection, max_projection)
}

#[cfg(test)]
mod tests {
  use super::{default_directional_options, normalize_direction};

  #[test]
  fn zero_direction_falls_back_to_x_axis() {
    assert_eq!(normalize_direction([0.0, 0.0]), [1.0, 0.0]);
  }

  #[test]
  fn defaults_span_image_projection() {
    let options = default_directional_options((100, 50), [1.0, 0.0]);
    assert_eq!(options.start, 0.0);
    assert_eq!(options.end, 100.0);
  }
}
