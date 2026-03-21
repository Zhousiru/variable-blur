use std::{
  path::PathBuf,
  sync::{
    mpsc::{self, Receiver, Sender},
    Arc,
  },
  thread,
  time::{Duration, Instant},
};

use eframe::{
  egui::{
    self, pos2, vec2, Button, Color32, ColorImage, ComboBox, Context, Grid, Rect, ScrollArea,
    Sense, Slider, Stroke, TextureHandle, TextureOptions,
  },
  App, CreationContext, Frame, NativeOptions,
};
use image::{DynamicImage, GenericImageView};
use rfd::FileDialog;
use variable_blur::core::{
  active_projection_span, apply_directional_variable_blur, auto_quality_settings,
  default_directional_options, generate_curve_anchors, generate_directional_step_map,
  quality_settings, AdvancedSettings, BlurCurve, DirectionalBlurOptions, PyramidConfig,
  VariableBlurConfig,
};

fn main() -> eframe::Result<()> {
  let options = NativeOptions::default();
  eframe::run_native(
    "Variable Blur Debug",
    options,
    Box::new(|cc| Ok(Box::new(DebugApp::new(cc)))),
  )
}

struct DebugApp {
  image_path: Option<PathBuf>,
  source_image: Option<Arc<DynamicImage>>,
  result_image: Option<DynamicImage>,
  source_texture: Option<TextureHandle>,
  result_texture: Option<TextureHandle>,
  params: UiParams,
  dirty: bool,
  rendering: bool,
  last_duration_ms: Option<f32>,
  error: Option<String>,
  next_request_id: u64,
  pending_request_id: Option<u64>,
  render_tx: Sender<RenderRequest>,
  render_rx: Receiver<RenderResult>,
  focused_image: Option<FocusedImage>,
}

impl DebugApp {
  fn new(cc: &CreationContext<'_>) -> Self {
    let (render_tx, render_rx) = spawn_render_worker(cc.egui_ctx.clone());
    Self {
      image_path: None,
      source_image: None,
      result_image: None,
      source_texture: None,
      result_texture: None,
      params: UiParams::default(),
      dirty: false,
      rendering: false,
      last_duration_ms: None,
      error: None,
      next_request_id: 0,
      pending_request_id: None,
      render_tx,
      render_rx,
      focused_image: None,
    }
  }

  fn load_image(&mut self, ctx: &Context, path: PathBuf) {
    match image::open(&path) {
      Ok(image) => {
        let image = Arc::new(image);
        self.image_path = Some(path);
        self.source_texture = Some(load_texture(ctx, "source", image.as_ref()));
        self.result_texture = None;
        self.result_image = None;
        self.error = None;
        self.last_duration_ms = None;
        self.focused_image = None;
        self.pending_request_id = None;
        self.rendering = false;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.source_image = Some(image);
        self.reset_range_to_image();
        self.dirty = true;
      }
      Err(err) => {
        self.error = Some(format!("load image failed: {err}"));
      }
    }
  }

  fn save_result(&mut self) {
    if let Some(result) = &self.result_image {
      if let Some(path) = FileDialog::new()
        .set_file_name("variable-blur.png")
        .save_file()
      {
        if let Err(err) = result.save(&path) {
          self.error = Some(format!("save image failed: {err}"));
        } else {
          self.error = None;
        }
      }
    }
  }

  fn reset_range_to_image(&mut self) {
    if let Some(image) = &self.source_image {
      let defaults =
        default_directional_options(image.dimensions(), [self.params.x, self.params.y]);
      self.params.start = defaults.start;
      self.params.end = defaults.end;
    }
  }

  fn poll_render_results(&mut self, ctx: &Context) {
    while let Ok(result) = self.render_rx.try_recv() {
      let Some(pending_id) = self.pending_request_id else {
        continue;
      };

      if result.id != pending_id {
        continue;
      }

      self.last_duration_ms = Some(result.duration_ms);
      self.result_texture = Some(load_texture(ctx, "result", &result.image));
      self.result_image = Some(result.image);
      self.error = None;
      self.rendering = false;
      self.pending_request_id = None;
    }
  }

  fn submit_render_if_needed(&mut self) {
    if !self.dirty {
      return;
    }

    let Some(source) = &self.source_image else {
      self.dirty = false;
      return;
    };

    let request_id = self.next_request_id;
    self.next_request_id = self.next_request_id.wrapping_add(1);
    let config = self.params.to_config(source.dimensions());
    let options = DirectionalBlurOptions {
      direction: [self.params.x, self.params.y],
      start: self.params.start,
      end: self.params.end,
    };

    match self.render_tx.send(RenderRequest {
      id: request_id,
      image: Arc::clone(source),
      config,
      options,
      show_step_map: self.params.show_step_map,
    }) {
      Ok(()) => {
        self.pending_request_id = Some(request_id);
        self.rendering = true;
        self.error = None;
      }
      Err(err) => {
        self.pending_request_id = None;
        self.rendering = false;
        self.error = Some(format!("queue render failed: {err}"));
      }
    }

    self.dirty = false;
  }
}

impl App for DebugApp {
  fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
    self.poll_render_results(ctx);

    if self.focused_image.is_some() && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.focused_image = None;
    }

    egui::SidePanel::left("controls")
      .resizable(true)
      .default_width(280.0)
      .show(ctx, |ui| {
        ui.heading("Variable Blur");

        if ui.button("Open image").clicked() {
          if let Some(path) = FileDialog::new()
            .add_filter(
              "images",
              &["png", "jpg", "jpeg", "webp", "bmp", "tiff", "tga"],
            )
            .pick_file()
          {
            self.load_image(ctx, path);
          }
        }

        if ui
          .add_enabled(
            self.result_image.is_some(),
            egui::Button::new("Save result"),
          )
          .clicked()
        {
          self.save_result();
        }

        if ui
          .add_enabled(
            self.source_image.is_some(),
            egui::Button::new("Reset range"),
          )
          .clicked()
        {
          self.reset_range_to_image();
          self.dirty = true;
        }

        if let Some(path) = &self.image_path {
          ui.label(path.display().to_string());
        }

        ui.separator();

        let mut changed = false;
        changed |= ui
          .checkbox(&mut self.params.show_step_map, "show stepMap")
          .changed();
        let direction_changed = draw_direction_pad(ui, &mut self.params);
        changed |= direction_changed;
        if direction_changed && self.source_image.is_some() {
          self.reset_range_to_image();
        }
        changed |= ui
          .add(Slider::new(&mut self.params.start, -4000.0..=4000.0).text("start"))
          .changed();
        changed |= ui
          .add(Slider::new(&mut self.params.end, -4000.0..=4000.0).text("end"))
          .changed();
        changed |= ui
          .add(Slider::new(&mut self.params.max_sigma, 1.0..=128.0).text("max sigma"))
          .changed();

        ui.collapsing("Curve", |ui| {
          ComboBox::from_label("curve")
            .selected_text(self.params.curve.label())
            .show_ui(ui, |ui| {
              changed |= ui
                .selectable_value(&mut self.params.curve, CurveMode::Linear, "Linear")
                .changed();
              changed |= ui
                .selectable_value(&mut self.params.curve, CurveMode::Power, "Power")
                .changed();
              changed |= ui
                .selectable_value(
                  &mut self.params.curve,
                  CurveMode::CubicBezier,
                  "CubicBezier",
                )
                .changed();
            });

          match self.params.curve {
            CurveMode::Power => {
              changed |= ui
                .add(Slider::new(&mut self.params.curve_gamma, 0.2..=4.0).text("curve gamma"))
                .changed();
            }
            CurveMode::CubicBezier => {
              changed |= ui
                .add(Slider::new(&mut self.params.curve_bezier_x1, 0.0..=1.0).text("x1"))
                .changed();
              changed |= ui
                .add(Slider::new(&mut self.params.curve_bezier_y1, 0.0..=1.0).text("y1"))
                .changed();
              changed |= ui
                .add(Slider::new(&mut self.params.curve_bezier_x2, 0.0..=1.0).text("x2"))
                .changed();
              changed |= ui
                .add(Slider::new(&mut self.params.curve_bezier_y2, 0.0..=1.0).text("y2"))
                .changed();
            }
            CurveMode::Linear => {}
          }

          let preview_curve = self.params.to_curve();
          let preview_steps = if self.params.auto_advanced {
            let preview_span = self.params.effective_blur_span(
              self
                .source_image
                .as_ref()
                .map(|image| image.dimensions()),
            );
            quality_settings(
              self.params.quality,
              &preview_curve,
              self.params.max_sigma,
              preview_span,
            )
            .steps
          } else {
            self.params.steps
          };

          ui.add_space(6.0);
          draw_curve_preview(ui, &preview_curve, preview_steps, self.params.max_sigma);
        });

        ui.collapsing("Advanced", |ui| {
          changed |= ui
            .checkbox(&mut self.params.auto_advanced, "auto advanced")
            .changed();
          ui.label(
            egui::RichText::new(
              "derive advanced parameters from image size, active blur span, curve shape, and max sigma",
            )
            .small(),
          );
          ui.add_space(6.0);

          let quality_changed = draw_quality_control(ui, &mut self.params.quality);
          changed |= quality_changed;

          if self.params.auto_advanced {
            if let Some(image) = &self.source_image {
              let derived = self.params.resolved_advanced(image.dimensions());
              draw_resolved_advanced(ui, &derived);
            } else {
              ui.label("Load an image to derive advanced values.");
            }
          } else {
            if quality_changed {
              self.params.apply_quality(
                self.params.quality,
                self
                  .source_image
                  .as_ref()
                  .map(|image| image.dimensions()),
              );
            }
            changed |= ui
              .add(Slider::new(&mut self.params.steps, 2..=24).text("steps"))
              .changed();
            changed |= ui
              .add(Slider::new(&mut self.params.max_levels, 1..=10).text("max levels"))
              .changed();
            changed |= draw_local_sigma_triplet(
              ui,
              &mut self.params.min_local_sigma,
              &mut self.params.target_local_sigma,
              &mut self.params.max_local_sigma,
            );
            changed |= ui
              .add(
                Slider::new(&mut self.params.downsample_stage_sigma, 0.1..=2.0)
                  .text("downsample sigma"),
              )
              .changed();
          }
        });

        if changed {
          self.dirty = true;
        }

        if let Some(duration) = self.last_duration_ms {
          ui.label(format!("render: {duration:.1} ms"));
        }
        if self.rendering {
          ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Rendering");
          });
        }
        if let Some(error) = &self.error {
          ui.colored_label(egui::Color32::RED, error);
        }
      });

    self.submit_render_if_needed();

    if self.rendering {
      ctx.request_repaint_after(Duration::from_millis(16));
    }

    egui::CentralPanel::default().show(ctx, |ui| match self.focused_image {
      Some(FocusedImage::Source) => {
        show_focused_panel(
          ui,
          "Source",
          self.source_texture.as_ref(),
          &mut self.focused_image,
        );
      }
      Some(FocusedImage::Result) => {
        show_focused_panel(
          ui,
          "Result",
          self.result_texture.as_ref(),
          &mut self.focused_image,
        );
      }
      None => {
        ui.columns(2, |columns| {
          columns[0].heading("Source");
          if let Some(texture) = &self.source_texture {
            let response = show_texture(&mut columns[0], texture);
            if response.clicked() {
              self.focused_image = Some(FocusedImage::Source);
            }
          }

          columns[1].heading("Result");
          if let Some(texture) = &self.result_texture {
            let response = show_texture(&mut columns[1], texture);
            if response.clicked() {
              self.focused_image = Some(FocusedImage::Result);
            }
          }
        });
      }
    });
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedImage {
  Source,
  Result,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CurveMode {
  Linear,
  Power,
  CubicBezier,
}

impl CurveMode {
  fn label(self) -> &'static str {
    match self {
      Self::Linear => "Linear",
      Self::Power => "Power",
      Self::CubicBezier => "CubicBezier",
    }
  }
}

#[derive(Clone)]
struct UiParams {
  quality: f32,
  auto_advanced: bool,
  show_step_map: bool,
  x: f32,
  y: f32,
  start: f32,
  end: f32,
  max_sigma: f32,
  steps: usize,
  curve: CurveMode,
  curve_gamma: f32,
  curve_bezier_x1: f32,
  curve_bezier_y1: f32,
  curve_bezier_x2: f32,
  curve_bezier_y2: f32,
  max_levels: usize,
  target_local_sigma: f32,
  min_local_sigma: f32,
  max_local_sigma: f32,
  downsample_stage_sigma: f32,
}

impl Default for UiParams {
  fn default() -> Self {
    let mut params = Self {
      quality: 0.5,
      auto_advanced: true,
      show_step_map: false,
      x: 1.0,
      y: 0.0,
      start: 0.0,
      end: 512.0,
      max_sigma: 32.0,
      steps: 12,
      curve: CurveMode::Power,
      curve_gamma: 1.6,
      curve_bezier_x1: 0.42,
      curve_bezier_y1: 0.0,
      curve_bezier_x2: 0.58,
      curve_bezier_y2: 1.0,
      max_levels: 5,
      target_local_sigma: 2.5,
      min_local_sigma: 0.5,
      max_local_sigma: 6.25,
      downsample_stage_sigma: 0.5,
    };
    params.apply_quality(0.5, None);
    params
  }
}

impl UiParams {
  fn to_config(&self, dimensions: (u32, u32)) -> VariableBlurConfig {
    let advanced = self.resolved_advanced(dimensions);
    let min_local_sigma = advanced.min_local_sigma.max(0.01);

    VariableBlurConfig {
      max_sigma: self.max_sigma,
      steps: advanced.steps,
      curve: self.to_curve(),
      pyramid: PyramidConfig {
        max_levels: advanced.max_levels.max(1),
        target_local_sigma: advanced.target_local_sigma.max(0.01),
        min_local_sigma,
        max_local_sigma: advanced.max_local_sigma.max(min_local_sigma),
        downsample_stage_sigma: advanced.downsample_stage_sigma.max(0.01),
      },
    }
  }

  fn apply_quality(&mut self, q: f32, dimensions: Option<(u32, u32)>) {
    self.quality = q.clamp(0.0, 1.0);
    let curve = self.to_curve();
    let values = quality_settings(
      self.quality,
      &curve,
      self.max_sigma,
      self.effective_blur_span(dimensions),
    );
    self.steps = values.steps;
    self.max_levels = values.max_levels;
    self.target_local_sigma = values.target_local_sigma;
    self.min_local_sigma = values.min_local_sigma;
    self.max_local_sigma = values.max_local_sigma;
    self.downsample_stage_sigma = values.downsample_stage_sigma;
  }

  fn to_curve(&self) -> BlurCurve {
    match self.curve {
      CurveMode::Linear => BlurCurve::Linear,
      CurveMode::Power => BlurCurve::Power(self.curve_gamma.max(0.01)),
      CurveMode::CubicBezier => BlurCurve::CubicBezier {
        x1: self.curve_bezier_x1.clamp(0.0, 1.0),
        y1: self.curve_bezier_y1.clamp(0.0, 1.0),
        x2: self.curve_bezier_x2.clamp(0.0, 1.0),
        y2: self.curve_bezier_y2.clamp(0.0, 1.0),
      },
    }
  }

  fn effective_blur_span(&self, dimensions: Option<(u32, u32)>) -> f32 {
    match dimensions {
      Some(dimensions) => {
        active_projection_span(dimensions, [self.x, self.y], self.start, self.end)
      }
      None => (self.end - self.start).abs(),
    }
  }

  fn resolved_advanced(&self, dimensions: (u32, u32)) -> AdvancedSettings {
    let curve = self.to_curve();
    let blur_span = self.effective_blur_span(Some(dimensions));
    if self.auto_advanced {
      auto_quality_settings(
        self.quality,
        &curve,
        dimensions,
        self.max_sigma.max(0.01),
        blur_span,
      )
    } else {
      let min_local_sigma = self.min_local_sigma.max(0.01);
      let max_local_sigma = self.max_local_sigma.max(min_local_sigma);
      let target_local_sigma = self
        .target_local_sigma
        .clamp(min_local_sigma, max_local_sigma);
      AdvancedSettings {
        steps: self.steps,
        max_levels: self.max_levels.max(1),
        target_local_sigma,
        min_local_sigma,
        max_local_sigma,
        downsample_stage_sigma: self.downsample_stage_sigma.max(0.01),
      }
    }
  }
}

fn load_texture(ctx: &Context, name: &str, image: &DynamicImage) -> TextureHandle {
  let rgba = image.to_rgba8();
  let size = [rgba.width() as usize, rgba.height() as usize];
  let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
  ctx.load_texture(name.to_owned(), color_image, TextureOptions::LINEAR)
}

fn draw_local_sigma_triplet(
  ui: &mut egui::Ui,
  min_local: &mut f32,
  target_local: &mut f32,
  max_local: &mut f32,
) -> bool {
  const RANGE_MIN: f32 = 0.1;
  const RANGE_MAX: f32 = 8.0;

  *min_local = min_local.clamp(RANGE_MIN, RANGE_MAX);
  *max_local = max_local.clamp(*min_local, RANGE_MAX);
  *target_local = target_local.clamp(*min_local, *max_local);

  ui.label("local sigma range");

  let desired_width = ui.available_width().max(180.0);
  let desired_size = vec2(desired_width, 56.0);
  let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());

  let track_margin = 14.0;
  let track_rect = Rect::from_min_max(
    pos2(rect.left() + track_margin, rect.center().y + 8.0),
    pos2(rect.right() - track_margin, rect.center().y + 12.0),
  );
  let value_to_x = |value: f32| {
    egui::remap_clamp(
      value,
      RANGE_MIN..=RANGE_MAX,
      track_rect.left()..=track_rect.right(),
    )
  };
  let x_to_value = |x: f32| {
    egui::remap_clamp(
      x,
      track_rect.left()..=track_rect.right(),
      RANGE_MIN..=RANGE_MAX,
    )
  };

  let mut changed = false;
  let mut handle = |ui: &mut egui::Ui,
                    id_suffix: &str,
                    value: &mut f32,
                    clamp_min: f32,
                    clamp_max: f32,
                    color: Color32| {
    let x = value_to_x(*value);
    let handle_rect = Rect::from_center_size(pos2(x, rect.center().y + 10.0), vec2(12.0, 24.0));
    let response = ui.interact(
      handle_rect,
      ui.make_persistent_id(("local_sigma_handle", id_suffix)),
      Sense::click_and_drag(),
    );

    if response.dragged() {
      if let Some(pointer_pos) = ui.input(|input| input.pointer.interact_pos()) {
        let new_value = x_to_value(pointer_pos.x).clamp(clamp_min, clamp_max);
        if (new_value - *value).abs() > f32::EPSILON {
          *value = new_value;
          changed = true;
        }
      }
    }

    ui.painter()
      .circle_filled(pos2(value_to_x(*value), rect.center().y + 10.0), 6.0, color);
  };

  ui.painter()
    .rect_filled(track_rect, 2.0, ui.visuals().widgets.inactive.bg_fill);
  ui.painter().line_segment(
    [
      pos2(value_to_x(*min_local), track_rect.center().y),
      pos2(value_to_x(*max_local), track_rect.center().y),
    ],
    Stroke::new(4.0, Color32::from_rgb(110, 170, 255)),
  );

  handle(
    ui,
    "min",
    min_local,
    RANGE_MIN,
    *target_local,
    Color32::from_rgb(100, 200, 140),
  );
  handle(
    ui,
    "target",
    target_local,
    *min_local,
    *max_local,
    Color32::from_rgb(250, 210, 90),
  );
  handle(
    ui,
    "max",
    max_local,
    *target_local,
    RANGE_MAX,
    Color32::from_rgb(240, 120, 120),
  );

  *max_local = max_local.clamp(*target_local, RANGE_MAX);
  *min_local = min_local.clamp(RANGE_MIN, *target_local);
  *target_local = target_local.clamp(*min_local, *max_local);

  ui.add_space(2.0);
  ui.horizontal(|ui| {
    ui.label(format!("min {:.2}", *min_local));
    ui.separator();
    ui.label(format!("target {:.2}", *target_local));
    ui.separator();
    ui.label(format!("max {:.2}", *max_local));
  });

  changed
}

fn draw_resolved_advanced(ui: &mut egui::Ui, advanced: &AdvancedSettings) {
  ui.label("resolved advanced values");
  Grid::new("resolved_advanced")
    .num_columns(2)
    .spacing([12.0, 4.0])
    .show(ui, |ui| {
      ui.label("steps");
      ui.monospace(advanced.steps.to_string());
      ui.end_row();

      ui.label("max levels");
      ui.monospace(advanced.max_levels.to_string());
      ui.end_row();

      ui.label("target local");
      ui.monospace(format!("{:.2}", advanced.target_local_sigma));
      ui.end_row();

      ui.label("min local");
      ui.monospace(format!("{:.2}", advanced.min_local_sigma));
      ui.end_row();

      ui.label("max local");
      ui.monospace(format!("{:.2}", advanced.max_local_sigma));
      ui.end_row();

      ui.label("downsample sigma");
      ui.monospace(format!("{:.2}", advanced.downsample_stage_sigma));
      ui.end_row();
    });
}

fn draw_direction_pad(ui: &mut egui::Ui, params: &mut UiParams) -> bool {
  let mut changed = false;
  ui.label("Direction");
  Grid::new("direction_pad")
    .num_columns(3)
    .spacing([6.0, 6.0])
    .show(ui, |ui| {
      changed |= direction_button(ui, params, "↖", [-1.0, -1.0]);
      changed |= direction_button(ui, params, "⬆", [0.0, -1.0]);
      changed |= direction_button(ui, params, "↗", [1.0, -1.0]);
      ui.end_row();

      changed |= direction_button(ui, params, "⬅", [-1.0, 0.0]);
      ui.label("");
      changed |= direction_button(ui, params, "➡", [1.0, 0.0]);
      ui.end_row();

      changed |= direction_button(ui, params, "↙", [-1.0, 1.0]);
      changed |= direction_button(ui, params, "⬇", [0.0, 1.0]);
      changed |= direction_button(ui, params, "↘", [1.0, 1.0]);
    });
  ui.label(format!("dir = ({:.1}, {:.1})", params.x, params.y));
  ui.add_space(8.0);
  changed
}

fn direction_button(
  ui: &mut egui::Ui,
  params: &mut UiParams,
  label: &str,
  direction: [f32; 2],
) -> bool {
  let selected = (params.x - direction[0]).abs() < 0.001 && (params.y - direction[1]).abs() < 0.001;
  if ui.add(Button::new(label).selected(selected)).clicked() {
    params.x = direction[0];
    params.y = direction[1];
    true
  } else {
    false
  }
}

fn draw_quality_control(ui: &mut egui::Ui, quality: &mut f32) -> bool {
  let mut changed = false;

  changed |= ui
    .add(
      Slider::new(quality, 0.0..=1.0)
        .text("quality")
        .fixed_decimals(2),
    )
    .changed();

  ui.horizontal(|ui| {
    for (label, q) in [("0.00", 0.0), ("0.50", 0.5), ("1.00", 1.0)] {
      let selected = (*quality - q).abs() < 0.01;
      if ui.add(Button::new(label).selected(selected)).clicked() {
        *quality = q;
        changed = true;
      }
    }
  });
  ui.add_space(6.0);

  changed
}

fn show_texture(ui: &mut egui::Ui, texture: &TextureHandle) -> egui::Response {
  let available = ui.available_size();
  let image_size = texture.size_vec2();
  if image_size.x <= 0.0 || image_size.y <= 0.0 {
    return ui.label("empty");
  }

  let scale = (available.x / image_size.x)
    .min(available.y / image_size.y)
    .clamp(0.05, 1.0);
  ui.add(egui::Image::new((texture.id(), image_size * scale)).sense(egui::Sense::click()))
}

fn draw_curve_preview(ui: &mut egui::Ui, curve: &BlurCurve, steps: usize, max_sigma: f32) {
  let desired_size = egui::vec2(ui.available_width().max(120.0), 120.0);
  let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
  let painter = ui.painter_at(rect);
  let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color);
  let guide = egui::Stroke::new(
    1.0,
    ui.visuals()
      .widgets
      .noninteractive
      .bg_fill
      .gamma_multiply(0.5),
  );

  painter.rect_stroke(rect, 4.0, guide, egui::StrokeKind::Outside);
  painter.line_segment([rect.left_bottom(), rect.left_top()], guide);
  painter.line_segment([rect.left_bottom(), rect.right_bottom()], guide);
  painter.line_segment([rect.left_bottom(), rect.right_top()], guide);

  let resolution = 64;
  let mut points = Vec::with_capacity(resolution + 1);
  for index in 0..=resolution {
    let t = index as f32 / resolution as f32;
    let y = curve.eval(t);
    points.push(egui::pos2(
      egui::lerp(rect.left()..=rect.right(), t),
      egui::lerp(rect.bottom()..=rect.top(), y),
    ));
  }

  painter.add(egui::Shape::line(points, stroke));
  let dot_color = ui.visuals().selection.bg_fill;

  for anchor in generate_curve_anchors(curve, steps.max(2), max_sigma) {
    let center = egui::pos2(
      egui::lerp(rect.left()..=rect.right(), anchor.t),
      egui::lerp(
        rect.bottom()..=rect.top(),
        (anchor.sigma / max_sigma.max(0.01)).clamp(0.0, 1.0),
      ),
    );
    painter.circle_filled(center, 2.5, dot_color);
  }
}

struct RenderRequest {
  id: u64,
  image: Arc<DynamicImage>,
  config: VariableBlurConfig,
  options: DirectionalBlurOptions,
  show_step_map: bool,
}

struct RenderResult {
  id: u64,
  image: DynamicImage,
  duration_ms: f32,
}

fn spawn_render_worker(ctx: Context) -> (Sender<RenderRequest>, Receiver<RenderResult>) {
  let (request_tx, request_rx) = mpsc::channel::<RenderRequest>();
  let (result_tx, result_rx) = mpsc::channel::<RenderResult>();

  thread::spawn(move || {
    while let Ok(mut request) = request_rx.recv() {
      while let Ok(newer_request) = request_rx.try_recv() {
        request = newer_request;
      }

      let started = Instant::now();
      let image = if request.show_step_map {
        generate_directional_step_map(request.image.dimensions(), request.config, request.options)
      } else {
        apply_directional_variable_blur(&request.image, request.config, request.options)
      };
      let duration_ms = started.elapsed().as_secs_f32() * 1000.0;

      if result_tx
        .send(RenderResult {
          id: request.id,
          image,
          duration_ms,
        })
        .is_err()
      {
        break;
      }

      ctx.request_repaint();
    }
  });

  (request_tx, result_rx)
}

fn show_focused_panel(
  ui: &mut egui::Ui,
  title: &str,
  texture: Option<&TextureHandle>,
  focused_image: &mut Option<FocusedImage>,
) {
  ui.horizontal(|ui| {
    ui.heading(title);
    if ui.button("Exit focus").clicked() {
      *focused_image = None;
    }
    ui.label("100% zoom");
    ui.label("Esc to exit");
  });
  ui.separator();

  match texture {
    Some(texture) => {
      ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
          let image_size = texture.size_vec2();
          let response =
            ui.add(egui::Image::new((texture.id(), image_size)).sense(egui::Sense::click()));
          if response.clicked() {
            *focused_image = None;
          }
        });
    }
    None => {
      ui.label("Image unavailable");
      if ui.button("Back").clicked() {
        *focused_image = None;
      }
    }
  }
}
