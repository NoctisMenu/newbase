use newoverlay::imgui::Ui;

impl<S: Send + Sync + 'static> crate::App<S> {
    /// Render the original newbase logo sequence. The schema-generated WebView
    /// menu is revealed by the overlay loop after this sequence completes.
    pub fn render_menu(&mut self, ui: &Ui) {
        if self.menu_intro_finished {
            return;
        }

        let elapsed = self.menu_intro_elapsed;
        const ANIM_SCALE: f32 = 2.0 / 3.0;
        const IN_DURATION: f32 = 1.10 * ANIM_SCALE;
        const HOLD_DURATION: f32 = 2.00 * ANIM_SCALE;
        const OUT_DURATION: f32 = 1.10 * ANIM_SCALE;
        const START_SCALE: f32 = 0.72;
        const END_SCALE: f32 = 1.0;

        let total_duration = IN_DURATION + HOLD_DURATION + OUT_DURATION;
        if elapsed >= total_duration {
            self.menu_intro_finished = true;
            return;
        }

        let smoothstep = |t: f32| t * t * (3.0 - 2.0 * t);
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let (alpha, scale) = if elapsed < IN_DURATION {
            let t = smoothstep((elapsed / IN_DURATION).clamp(0.0, 1.0));
            (t, lerp(START_SCALE, END_SCALE, t))
        } else if elapsed < IN_DURATION + HOLD_DURATION {
            (1.0, END_SCALE)
        } else {
            let t = smoothstep(
                ((elapsed - IN_DURATION - HOLD_DURATION) / OUT_DURATION).clamp(0.0, 1.0),
            );
            (1.0 - t, lerp(END_SCALE, START_SCALE, t))
        };

        let [window_w, window_h] = ui.io().display_size;
        if window_w <= 0.0 || window_h <= 0.0 {
            return;
        }

        let center_x = window_w * 0.5;
        let center_y = window_h * 0.5;
        if let Some(logo) = &self.menu_logo {
            let base_w = (window_w * 0.32).clamp(180.0, 420.0);
            let base_h = base_w / logo.aspect_ratio.max(0.01);
            let draw_w = base_w * scale;
            let draw_h = base_h * scale;
            let p_min = [center_x - draw_w * 0.5, center_y - draw_h * 0.5];
            let p_max = [center_x + draw_w * 0.5, center_y + draw_h * 0.5];

            ui.get_foreground_draw_list()
                .add_image(logo.texture_id, p_min, p_max)
                .uv_min(logo.uv_min)
                .uv_max(logo.uv_max)
                .col([1.0, 1.0, 1.0, alpha])
                .build();
        } else {
            let text = "Initializing...";
            let [text_w, text_h] = ui.calc_text_size(text);
            ui.get_foreground_draw_list().add_text(
                [center_x - text_w * 0.5, center_y - text_h * 0.5],
                [1.0, 1.0, 1.0, alpha],
                text,
            );
        }
    }
}
