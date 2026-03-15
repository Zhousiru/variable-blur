#[derive(Clone)]
pub(crate) struct LinearRgbaImage {
  pub(crate) width: usize,
  pub(crate) height: usize,
  pub(crate) data: Vec<[f32; 4]>,
}

impl LinearRgbaImage {
  pub(crate) fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      data: vec![[0.0; 4]; width * height],
    }
  }
}
