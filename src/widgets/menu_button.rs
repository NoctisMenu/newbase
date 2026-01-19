use egui::{self, *};
use std::time::{Duration, Instant};

use crate::widgets::{Animation, DEFAULT_SPRING};

pub struct MenuButton {
    pub selected: bool,
    pub texture: Option<TextureId>,
    pub size: Vec2,

    animation: Animation,

    // visuals
    line_thickness: f32,
    pub line_color: Color32,
}

impl Default for MenuButton {
    fn default() -> Self {
        Self {
            selected: false,
            texture: None,
            size: Vec2::new(80.0, 80.0),
            animation: Animation::new(Duration::from_millis(750), Some(DEFAULT_SPRING)),
            line_thickness: 2.0,
            line_color: Color32::from_rgb(144, 238, 144),
        }
    }
}

impl MenuButton {
    pub fn new(texture: Option<TextureId>) -> Self {
        let mut s = Self::default();
        s.texture = texture;
        s
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn with_line_color(mut self, color: Color32) -> Self {
        self.line_color = color;
        self
    }

    pub fn with_line_thickness(mut self, t: f32) -> Self {
        self.line_thickness = t;
        self
    }

    pub fn with_animation(mut self, animation: Animation) -> Self {
        self.animation = animation;
        self
    }

    /// externally set selection state; will start animation
    pub fn set_selected(&mut self, selected: bool) {
        if self.selected != selected {
            self.selected = selected;
            self.animation.start(Instant::now());
        }
    }

    fn update_animation(&mut self, ctx: &Context) {
        self.animation.update();
        ctx.request_repaint();
    }

    /// Draw the menu button (image centered) and an animated thin underline when selected.
    /// Clicking the button will mark it selected (and start animation). If you want exclusive selection
    /// across multiple buttons, manage that from the caller (use set_selected on others).
    pub fn show(&mut self, ui: &mut Ui, label: &str) -> Response {
        self.update_animation(ui.ctx());
        let progress = if self.selected {
            self.animation.progress
        } else {
            1.0 - self.animation.progress
        }
        .max(-1.0);

        let (rect, mut response) =
            ui.allocate_exact_size(self.size * (progress / 2.0 + 0.5), Sense::click_and_drag()); // block window moving when user drags button

        if response.clicked() && !self.selected {
            // mark selected and start animation
            self.selected = true;
            self.animation.start(Instant::now());
            response.mark_changed();
            ui.ctx().request_repaint();
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter().clone();

            // background hit-state / hover visuals (optional)
            //let visuals = ui.style().interact(&response);
            let rounding = Rounding::same(rect.height() * 0.12);
            painter.rect_stroke(rect, rounding, Stroke::new(1.0, self.line_color));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::default(),
                self.line_color,
            );

            // animated underline
            if progress > 0.0 {
                let full_w = rect.width() - (rect.width() / 5.0);
                let line_w = full_w * progress.min(1.0).max(0.0);
                let x_center = rect.center().x;
                let left = x_center - line_w / 2.0;
                let top = rect.bottom() - self.line_thickness - 10.0; // small margin from bottom
                let line_rect =
                    Rect::from_min_size(Pos2::new(left, top), vec2(line_w, self.line_thickness));
                painter.rect_filled(
                    line_rect,
                    Rounding::same(self.line_thickness / 2.0),
                    self.line_color,
                );
            }
        }

        response
    }
}
