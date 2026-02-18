use newoverlay::imgui::{
    ChildWindow, Condition, Selectable, Slider, StyleColor, StyleVar, Ui, Window,
};

const SIDEBAR_ITEMS: [&str; 6] = [
    "Visuals",
    "Misc",
    "Assist",
    "Radar",
    "User Profiles",
    "Settings",
];
const LANGUAGE_CHOICES: [&str; 4] = ["English", "Spanish", "German", "Japanese"];
const RENDER_SCALE_CHOICES: [&str; 4] = ["75%", "100%", "125%", "150%"];

fn sidebar_entry(ui: &Ui, label: &str, selected: bool) -> bool {
    let (header, hovered, active, text_col) = if selected {
        (
            [0.16, 0.13, 0.08, 0.95],
            [0.23, 0.19, 0.11, 1.0],
            [0.25, 0.21, 0.12, 1.0],
            [0.95, 0.76, 0.32, 1.0],
        )
    } else {
        (
            [0.06, 0.07, 0.10, 0.0],
            [0.13, 0.15, 0.20, 0.60],
            [0.14, 0.17, 0.24, 0.80],
            [0.78, 0.81, 0.89, 1.0],
        )
    };

    let _header = ui.push_style_color(StyleColor::Header, header);
    let _header_hovered = ui.push_style_color(StyleColor::HeaderHovered, hovered);
    let _header_active = ui.push_style_color(StyleColor::HeaderActive, active);
    let _text = ui.push_style_color(StyleColor::Text, text_col);

    Selectable::new(label)
        .selected(selected)
        .size([0.0, 30.0])
        .build(ui)
}

fn section_card(ui: &Ui, id: &str, title: &str, height: f32, f: impl FnOnce(&Ui)) {
    let _round = ui.push_style_var(StyleVar::ChildRounding(8.0));
    let _padding = ui.push_style_var(StyleVar::WindowPadding([12.0, 12.0]));
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([8.0, 8.0]));
    let _bg = ui.push_style_color(StyleColor::ChildBg, [0.08, 0.09, 0.13, 0.92]);
    let _border = ui.push_style_color(StyleColor::Border, [0.20, 0.22, 0.30, 0.90]);

    ChildWindow::new(id)
        .size([0.0, height])
        .border(true)
        .build(ui, || {
            ui.text_colored([0.96, 0.73, 0.28, 1.0], title);
            ui.separator();
            f(ui);
        });
}

fn keybind_row(ui: &Ui, label: &str, button_label: &str, id_suffix: &str) {
    let row_start_x = ui.cursor_pos()[0];
    ui.text(label);
    ui.same_line_with_pos(row_start_x + 155.0);

    let _btn = ui.push_style_color(StyleColor::Button, [0.18, 0.20, 0.26, 0.95]);
    let _btn_hover = ui.push_style_color(StyleColor::ButtonHovered, [0.25, 0.28, 0.36, 1.0]);
    let _btn_active = ui.push_style_color(StyleColor::ButtonActive, [0.29, 0.32, 0.40, 1.0]);
    let _ = ui.button_with_size(format!("{}##{}", button_label, id_suffix), [86.0, 0.0]);
}

fn segmented_button(ui: &Ui, label: &str, active: bool, size: [f32; 2]) -> bool {
    let (base, hover, click, text_col) = if active {
        (
            [0.31, 0.33, 0.86, 0.95],
            [0.35, 0.37, 0.95, 1.0],
            [0.29, 0.31, 0.78, 1.0],
            [0.95, 0.96, 1.0, 1.0],
        )
    } else {
        (
            [0.17, 0.18, 0.25, 0.95],
            [0.22, 0.24, 0.32, 1.0],
            [0.25, 0.27, 0.37, 1.0],
            [0.78, 0.80, 0.90, 1.0],
        )
    };

    let _btn = ui.push_style_color(StyleColor::Button, base);
    let _btn_hover = ui.push_style_color(StyleColor::ButtonHovered, hover);
    let _btn_active = ui.push_style_color(StyleColor::ButtonActive, click);
    let _text = ui.push_style_color(StyleColor::Text, text_col);
    ui.button_with_size(label, size)
}

impl<S: Send + Sync + 'static> crate::App<S> {
    pub fn render_menu(&mut self, ui: &Ui) {
        if !self.menu_intro_finished {
            let elapsed = self.menu_intro_elapsed;
            const START_DELAY: f32 = 0.0;
            const ANIM_SCALE: f32 = 2.0 / 3.0;
            const IN_DURATION: f32 = 1.10 * ANIM_SCALE;
            const HOLD_DURATION: f32 = 2.00 * ANIM_SCALE;
            const OUT_DURATION: f32 = 1.10 * ANIM_SCALE;
            const START_SCALE: f32 = 0.72;
            const END_SCALE: f32 = 1.0;

            let total_duration = START_DELAY + IN_DURATION + HOLD_DURATION + OUT_DURATION;
            if elapsed >= total_duration {
                self.menu_intro_finished = true;
                return;
            }
            if elapsed < START_DELAY {
                return;
            }
            let anim_elapsed = elapsed - START_DELAY;

            let smoothstep = |t: f32| t * t * (3.0 - 2.0 * t);
            let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

            let (alpha, scale) = if anim_elapsed < IN_DURATION {
                let t = smoothstep((anim_elapsed / IN_DURATION).clamp(0.0, 1.0));
                (t, lerp(START_SCALE, END_SCALE, t))
            } else if anim_elapsed < IN_DURATION + HOLD_DURATION {
                (1.0, END_SCALE)
            } else {
                let t = smoothstep(
                    ((anim_elapsed - IN_DURATION - HOLD_DURATION) / OUT_DURATION).clamp(0.0, 1.0),
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
                let [tw, th] = ui.calc_text_size(text);
                ui.get_foreground_draw_list().add_text(
                    [center_x - tw * 0.5, center_y - th * 0.5],
                    [1.0, 1.0, 1.0, alpha],
                    text,
                );
            }

            return;
        }

        let [display_w, display_h] = ui.io().display_size;
        if display_w <= 0.0 || display_h <= 0.0 {
            return;
        }
        return;//fixme

        let menu_w = (display_w * 0.76).clamp(900.0, 1160.0);
        let menu_h = (display_h * 0.74).clamp(560.0, 760.0);

        let _window_round = ui.push_style_var(StyleVar::WindowRounding(10.0));
        let _window_border = ui.push_style_var(StyleVar::WindowBorderSize(1.0));
        let _frame_round = ui.push_style_var(StyleVar::FrameRounding(5.0));
        let _frame_pad = ui.push_style_var(StyleVar::FramePadding([8.0, 6.0]));
        let _grab_round = ui.push_style_var(StyleVar::GrabRounding(6.0));
        let _item_spacing = ui.push_style_var(StyleVar::ItemSpacing([8.0, 9.0]));

        let _window_bg = ui.push_style_color(StyleColor::WindowBg, [0.04, 0.05, 0.08, 0.94]);
        let _child_bg = ui.push_style_color(StyleColor::ChildBg, [0.06, 0.07, 0.11, 0.93]);
        let _border = ui.push_style_color(StyleColor::Border, [0.17, 0.19, 0.28, 0.95]);
        let _text = ui.push_style_color(StyleColor::Text, [0.90, 0.92, 0.97, 1.0]);
        let _frame_bg = ui.push_style_color(StyleColor::FrameBg, [0.13, 0.14, 0.20, 0.95]);
        let _frame_hover = ui.push_style_color(StyleColor::FrameBgHovered, [0.17, 0.18, 0.26, 1.0]);
        let _frame_active = ui.push_style_color(StyleColor::FrameBgActive, [0.20, 0.22, 0.31, 1.0]);
        let _check_mark = ui.push_style_color(StyleColor::CheckMark, [0.37, 0.42, 0.95, 1.0]);
        let _slider_grab = ui.push_style_color(StyleColor::SliderGrab, [0.41, 0.46, 1.0, 1.0]);
        let _slider_grab_active =
            ui.push_style_color(StyleColor::SliderGrabActive, [0.49, 0.54, 1.0, 1.0]);

        Window::new("##newbase_menu_main")
            .size([menu_w, menu_h], Condition::Always)
            .position([display_w * 0.5, display_h * 0.5], Condition::Always)
            .position_pivot([0.5, 0.5])
            .title_bar(false)
            .resizable(false)
            .movable(false)
            .collapsible(false)
            .scroll_bar(false)
            .save_settings(false)
            .build(ui, || {
                let win_pos = ui.window_pos();
                let win_size = ui.window_size();
                let dl = ui.get_window_draw_list();
                dl.add_rect_filled_multicolor(
                    [win_pos[0], win_pos[1]],
                    [win_pos[0] + win_size[0], win_pos[1] + win_size[1]],
                    [0.05, 0.06, 0.10, 0.94],
                    [0.04, 0.05, 0.11, 0.94],
                    [0.03, 0.04, 0.08, 0.97],
                    [0.03, 0.04, 0.08, 0.97],
                );
                dl.add_circle(
                    [
                        win_pos[0] + win_size[0] * 0.84,
                        win_pos[1] + win_size[1] * 0.14,
                    ],
                    180.0,
                    [0.23, 0.17, 0.36, 0.17],
                )
                .filled(true)
                .build();
                dl.add_circle(
                    [
                        win_pos[0] + win_size[0] * 0.18,
                        win_pos[1] + win_size[1] * 0.87,
                    ],
                    210.0,
                    [0.12, 0.16, 0.33, 0.16],
                )
                .filled(true)
                .build();
                dl.add_line(
                    [win_pos[0] + 206.0, win_pos[1] + 12.0],
                    [win_pos[0] + 206.0, win_pos[1] + win_size[1] - 12.0],
                    [0.18, 0.20, 0.29, 1.0],
                )
                .thickness(1.0)
                .build();

                ui.columns(2, "##main_layout", false);
                ui.set_column_width(0, 194.0);

                let _sidebar_pad = ui.push_style_var(StyleVar::WindowPadding([10.0, 10.0]));
                ChildWindow::new("##sidebar_panel")
                    .size([0.0, 0.0])
                    .border(false)
                    .build(ui, || {
                        ui.text_colored([0.85, 0.89, 0.97, 1.0], "Navigation");
                        ui.dummy([0.0, 6.0]);

                        for (idx, item) in SIDEBAR_ITEMS.iter().enumerate() {
                            let clicked =
                                sidebar_entry(ui, item, self.menu_ui_state.selected_sidebar == idx);
                            if clicked {
                                self.menu_ui_state.selected_sidebar = idx;
                            }
                        }

                        let remaining_h = ui.content_region_avail()[1];
                        if remaining_h > 34.0 {
                            ui.dummy([0.0, remaining_h - 30.0]);
                        }

                        let _btn =
                            ui.push_style_color(StyleColor::Button, [0.20, 0.08, 0.08, 0.80]);
                        let _btn_hover = ui
                            .push_style_color(StyleColor::ButtonHovered, [0.28, 0.11, 0.11, 0.95]);
                        let _btn_active =
                            ui.push_style_color(StyleColor::ButtonActive, [0.32, 0.12, 0.12, 1.0]);
                        if ui.button_with_size("Exit", [0.0, 24.0]) {
                            self.request_shutdown();
                        }
                    });

                ui.next_column();

                let _content_pad = ui.push_style_var(StyleVar::WindowPadding([14.0, 10.0]));
                ChildWindow::new("##settings_content")
                    .size([0.0, 0.0])
                    .border(false)
                    .build(ui, || {
                        let selected_name = SIDEBAR_ITEMS
                            .get(self.menu_ui_state.selected_sidebar)
                            .copied()
                            .unwrap_or("Settings");
                        ui.text_colored([0.92, 0.94, 0.99, 1.0], selected_name);
                        ui.dummy([0.0, 10.0]);

                        if self.menu_ui_state.selected_sidebar != 5 {
                            ui.text_disabled("Temporary placeholder page.");
                            ui.text_disabled("Select Settings to see the full layout.");
                            return;
                        }

                        ui.columns(2, "##settings_grid", false);
                        let grid_w = ui.content_region_avail()[0];
                        ui.set_column_width(0, (grid_w * 0.52).max(280.0));

                        section_card(ui, "##language_card", "Language", 112.0, |ui| {
                            ui.text("Language");
                            ui.set_next_item_width(-1.0);
                            let _ = ui.combo_simple_string(
                                "##language_select",
                                &mut self.menu_ui_state.language_idx,
                                &LANGUAGE_CHOICES,
                            );
                        });

                        section_card(ui, "##input_card", "Input", 206.0, |ui| {
                            keybind_row(ui, "Menu key (?)", "Delete", "menu_key");
                            keybind_row(ui, "Shutdown key (?)", "End", "shutdown_key");
                            keybind_row(ui, "Hide key (?)", "Home", "hide_key");
                            ui.dummy([0.0, 6.0]);

                            let row_start_x = ui.cursor_pos()[0];
                            ui.text("Block input (?)");
                            ui.same_line_with_pos(row_start_x + 155.0);
                            let _ = ui.checkbox(
                                "##block_input_toggle",
                                &mut self.menu_ui_state.block_input,
                            );
                        });

                        ui.next_column();

                        section_card(ui, "##render_card", "Rendering", 145.0, |ui| {
                            ui.text("Scale");
                            ui.set_next_item_width(-1.0);
                            let _ = ui.combo_simple_string(
                                "##render_scale",
                                &mut self.menu_ui_state.render_scale_idx,
                                &RENDER_SCALE_CHOICES,
                            );

                            ui.dummy([0.0, 2.0]);
                            ui.text("Background Blur");
                            ui.set_next_item_width(-44.0);
                            let _ = Slider::new("##blur_strength", 0.0f32, 5.0f32)
                                .display_format("%.2f")
                                .build(ui, &mut self.menu_ui_state.blur_strength);
                            ui.same_line();
                            ui.text(format!("{:.2}", self.menu_ui_state.blur_strength));
                        });

                        section_card(ui, "##aiming_card", "Aiming", 102.0, |ui| {
                            ui.text("Controller");
                            let mouse_active = self.menu_ui_state.controller_idx == 0;
                            let angle_active = self.menu_ui_state.controller_idx == 1;

                            if segmented_button(
                                ui,
                                "Mouse (default)##mouse",
                                mouse_active,
                                [132.0, 26.0],
                            ) {
                                self.menu_ui_state.controller_idx = 0;
                            }
                            ui.same_line();
                            if segmented_button(ui, "Angle aim##angle", angle_active, [98.0, 26.0])
                            {
                                self.menu_ui_state.controller_idx = 1;
                            }
                        });

                        ui.columns(1, "##settings_grid_end", false);
                    });

                ui.columns(1, "##main_layout_end", false);
            });
    }
}
