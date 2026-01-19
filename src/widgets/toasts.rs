use std::time::{Duration, Instant};

use egui::{
    Color32, Pos2, Rect, Rgba, Stroke, Vec2,
};

pub struct Toast {
    pub start: Instant,
    pub text: String,
    pub duration: Duration,
    pub opacity: f32,
}

pub struct Toasts {
    list: Vec<Toast>,
}

impl Toasts {
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }
    pub fn show(&mut self, ctx: &egui::Context, painter: egui::Painter, accent_color: Color32) {
        let mut offset = 0.0;
        for toast in &mut self.list {
            if toast.start.elapsed() > toast.duration {
                toast.opacity = (500
                    - (toast.start.elapsed().as_millis() - toast.duration.as_millis()) as i128)
                    as f32
                    / 500.0;
            }
            if toast.opacity <= 0.0 {
                continue;
            }
            let progress =
                (toast.start.elapsed().as_secs_f32() / toast.duration.as_secs_f32()).min(1.0);
            let toast_x = if progress < 0.1 {
                1600.0 * progress - 150.0
            } else {
                10.0
            };

            let galley = ctx.fonts(|fonts| {
                fonts.layout_no_wrap(
                    toast.text.to_string(),
                    egui::FontId::proportional(16.0),
                    Rgba::from_white_alpha(toast.opacity).into(),
                )
            });
            let toast_rect = Rect::from_min_size(
                (toast_x, 10.0 + offset - (1.0 - toast.opacity) * 60.0).into(),
                Vec2::new(galley.size().x + 15.0, 50.0),
            );
            painter.rect(
                toast_rect,
                5.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, (toast.opacity * 200.0) as u8),
                Stroke::NONE,
            );
            let line_rect = Rect::from_min_max(
                Pos2::new(toast_rect.min.x, toast_rect.max.y - 5.0),
                Pos2::new(
                    toast_rect.min.x + (toast_rect.max.x - toast_rect.min.x) * progress,
                    toast_rect.max.y,
                ),
            );
            painter.rect_filled(line_rect, 10.0, accent_color.gamma_multiply(toast.opacity));
            painter.galley(
                Pos2::new(toast_rect.min.x + 10.0, toast_rect.center().y),
                galley,
                Rgba::from_white_alpha(toast.opacity).into(),
            );

            offset += 55.0 * toast.opacity;
        }
    }
    pub fn add_toast(&mut self, text: impl ToString, duration: Duration) {
        self.list.push(Toast {
            start: Instant::now(),
            text: text.to_string(),
            duration,
            opacity: 1.0,
        });
    }
}
