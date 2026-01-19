use crate::widgets::{Animation, DEFAULT_SPRING};
use egui::{self, *};
use std::time::{Duration, Instant};

use super::theming::Colors;

pub struct ToggleSwitch {
    pub enabled: bool,
    pub animation: Animation,
    size: Vec2,
    pub colors: Colors,
    pub highlight: bool,
}

impl Default for ToggleSwitch {
    fn default() -> Self {
        Self {
            enabled: false,
            animation: Animation::new(Duration::from_millis(700), Some(DEFAULT_SPRING)),
            size: Vec2::new(40.0, 20.0),
            colors: Colors::default(),
            highlight: false,
        }
    }
}

impl ToggleSwitch {
    pub fn new(enabled: bool, animation: Option<Animation>) -> Self {
        Self {
            enabled,
            animation: animation.unwrap_or_else(|| {
                Animation::new(Duration::from_millis(700), Some(DEFAULT_SPRING))
            }),
            ..Default::default()
        }
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn with_colors(mut self, colors: Colors) -> Self {
        self.colors = colors;
        self
    }

    /// Update animation state
    fn update_animation(&mut self, ctx: &Context) {
        self.animation.update();
        ctx.request_repaint();
    }

    /// Interpolate between two colors
    fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);
        Color32::from_rgba_premultiplied(
            (a.r() as f32 * (1.0 - t) + b.r() as f32 * t) as u8,
            (a.g() as f32 * (1.0 - t) + b.g() as f32 * t) as u8,
            (a.b() as f32 * (1.0 - t) + b.b() as f32 * t) as u8,
            (a.a() as f32 * (1.0 - t) + b.a() as f32 * t) as u8,
        )
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.animation.start(Instant::now());
        self.animation.set_values((
            if self.enabled { 1.0 } else { 0.0 },
            self.animation.progress,
        ));
    }

    pub fn set(&mut self, enabled: bool) {
        if enabled != self.enabled {
            self.toggle();
        }
    }

    pub fn display(&mut self, ui: &mut Ui, text: impl Into<String>, color: Color32) {
        let inner = ui.horizontal(|ui| {
            ui.label(
                RichText::new(text)
                    .color(Color32::GRAY.lerp_to_gamma(Color32::WHITE, self.animation.progress)),
            );
            ui.add_space(90.0);
            self.colors = Colors::default().set_on_color(color);
            self.show(ui);
        });
        if inner.response.hovered() {
            self.highlight = false;
        }
        if self.highlight {
            ui.painter().rect(
                inner.response.rect.expand(5.0),
                3.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 4),
                (0.0, Color32::TRANSPARENT),
            );
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        self.update_animation(ui.ctx());

        let (rect, mut response) = ui.allocate_exact_size(self.size, Sense::click_and_drag()); // block window moving when user drags button

        if response.clicked() {
            self.toggle();
            response.mark_changed();
            ui.ctx().request_repaint();
        }

        if ui.is_rect_visible(rect) {
            //let visuals = ui.style().interact(&response);

            // Calculate dimensions
            let height = rect.height();
            //let width = rect.width();
            let radius = height * 0.5;
            let handle_radius = radius * 0.75;

            let progress = self.animation.progress;

            // Background color interpolation
            let bg_color = Self::lerp_color(
                self.colors.background_off,
                self.colors.background_on,
                progress.clamp(0.0, 1.0),
            );

            // Draw background (rounded rectangle)
            ui.painter().rect(
                rect,
                Rounding::same(radius),
                bg_color,
                Stroke::new(1.0, bg_color),
            );

            // Calculate handle position
            let handle_start_x = rect.left() + radius;
            let handle_end_x = rect.right() - radius;
            let handle_x = handle_start_x + (handle_end_x - handle_start_x) * progress;
            let handle_center = Pos2::new(handle_x, rect.center().y);

            // Add shadow for handle
            let shadow_offset = Vec2::new(0.0, 1.0);
            let shadow_color = Color32::from_black_alpha(30);
            ui.painter()
                .circle_filled(handle_center + shadow_offset, handle_radius, shadow_color);

            // Draw handle
            let handle_color = if self.enabled || response.hovered() {
                Color32::from_rgb(245, 245, 245)
            } else {
                self.colors.handle_color
            };

            ui.painter()
                .circle_filled(handle_center, handle_radius, handle_color);
        }

        response
    }
}
