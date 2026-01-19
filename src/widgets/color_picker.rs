use egui::color_picker::{Alpha, show_color};
use egui::epaint::{Hsva, HsvaGamma};
use egui::{self, *};

fn color_button(ui: &mut Ui, color: Color32, open: bool, circle_size: f32) -> Response {
    let size = Vec2::new(25.0, 25.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));

    if ui.is_rect_visible(rect) {
        let visuals = if open {
            &ui.visuals().widgets.open
        } else {
            ui.style().interact(&response)
        };
        let rect = rect.expand(visuals.expansion);

        ui.painter()
            .circle_filled(rect.center(), circle_size / 2.0, color);
    }

    response
}

fn add_button(ui: &mut Ui, color: Color32, open: bool) -> Response {
    let size = Vec2::new(25.0, 25.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));

    if ui.is_rect_visible(rect) {
        let visuals = if open {
            &ui.visuals().widgets.open
        } else {
            ui.style().interact(&response)
        };
        let rect = rect.expand(visuals.expansion);

        ui.painter()
            .circle_filled(rect.center(), size.x / 2.0, color);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "+",
            FontId::new(30.0, FontFamily::Monospace),
            Color32::WHITE,
        );
    }

    response
}

fn contrast_color(color: impl Into<Rgba>) -> Color32 {
    if color.into().intensity() < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

fn color_slider_1d(ui: &mut Ui, value: &mut f32, color_at: impl Fn(f32) -> Color32) -> Response {
    #![allow(clippy::identity_op)]

    let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        //let visuals = ui.style().interact(&response);

        {
            // fill color with rounded corners:
            let mut mesh = Mesh::default();
            for i in 0..=36 {
                let t = i as f32 / 36.0;
                let color = color_at(t);
                let x = lerp(rect.left()..=rect.right(), t);
                mesh.colored_vertex(pos2(x, rect.top()), color);
                mesh.colored_vertex(pos2(x, rect.bottom()), color);
                if i < 36 {
                    mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                    mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
                }
            }
            //let rounding = rect.height() / 2.0;
            let shape = Shape::mesh(mesh);
            ui.painter().add(shape);
        }

        {
            // Show where the slider is at:
            let x = lerp(rect.left()..=rect.right(), *value);
            let pointer_width = rect.height() * 0.7;
            let pointer_height = rect.height() * 1.2;
            let pointer_rect = Rect::from_center_size(
                pos2(x, rect.center().y),
                vec2(pointer_width, pointer_height),
            );
            ui.painter()
                .rect_filled(pointer_rect, pointer_height / 10.0, Color32::WHITE);
        }
    }

    response
}

/// # Arguments
/// * `x_value` - X axis, either saturation or value (0.0-1.0).
/// * `y_value` - Y axis, either saturation or value (0.0-1.0).
/// * `color_at` - A function that dictates how the mix of saturation and value will be displayed in the 2d slider.
/// E.g.: `|x_value, y_value| HsvaGamma { h: 1.0, s: x_value, v: y_value, a: 1.0 }.into()` displays the colors as follows: top-left: white \[s: 0.0, v: 1.0], top-right: fully saturated color \[s: 1.0, v: 1.0], bottom-right: black \[s: 0.0, v: 1.0].
///
fn color_slider_2d(
    ui: &mut Ui,
    x_value: &mut f32,
    y_value: &mut f32,
    color_at: impl Fn(f32, f32) -> Color32,
) -> Response {
    let desired_size = Vec2::splat(ui.spacing().slider_width);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *x_value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
        *y_value = remap_clamp(mpos.y, rect.bottom()..=rect.top(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let mut mesh = Mesh::default();

        for xi in 0..=36 {
            for yi in 0..=36 {
                let xt = xi as f32 / (36 as f32);
                let yt = yi as f32 / (36 as f32);
                let color = color_at(xt, yt);
                let x = lerp(rect.left()..=rect.right(), xt);
                let y = lerp(rect.bottom()..=rect.top(), yt);
                mesh.colored_vertex(pos2(x, y), color);

                if xi < 36 && yi < 36 {
                    let x_offset = 1;
                    let y_offset = 36 + 1;
                    let tl = yi * y_offset + xi;
                    mesh.add_triangle(tl, tl + x_offset, tl + y_offset);
                    mesh.add_triangle(tl + x_offset, tl + y_offset, tl + y_offset + x_offset);
                }
            }
        }
        ui.painter().add(Shape::mesh(mesh)); // fill

        ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

        // Show where the slider is at:
        let x = lerp(rect.left()..=rect.right(), *x_value);
        let y = lerp(rect.bottom()..=rect.top(), *y_value);
        let picked_color = color_at(*x_value, *y_value);
        ui.painter().add(epaint::CircleShape {
            center: pos2(x, y),
            radius: rect.width() / 12.0,
            fill: picked_color,
            stroke: Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        });
    }

    response
}

fn color_picker_hsvag_2d(ui: &mut Ui, hsvag: &mut HsvaGamma, alpha: Alpha, hex_code: &mut String) {
    let current_color_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    show_color(ui, *hsvag, current_color_size).on_hover_text("Selected color");

    let opaque = HsvaGamma { a: 1.0, ..*hsvag };

    let HsvaGamma { h, s, v, a: _ } = hsvag;

    color_slider_2d(ui, s, v, |s, v| HsvaGamma { s, v, ..opaque }.into());

    color_slider_1d(ui, h, |h| {
        HsvaGamma {
            h,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        }
        .into()
    })
    .on_hover_text("Hue");
    //*hex_code = {let mut hex_code = Color32::from(Hsva::from(*hsvag)).to_hex();hex_code[0..hex_code.len()-2].to_string()};
    if ui.text_edit_singleline(hex_code).changed() {
        if let Ok(color) = Color32::from_hex(&hex_code) {
            dbg!(&hex_code);
            dbg!(color.to_srgba_unmultiplied());
            *hsvag = HsvaGamma::from(color);
        }
    };

    if alpha == Alpha::Opaque {
        hsvag.a = 1.0;
    }
}

fn color_picker_hsva_2d(ui: &mut Ui, hsva: &mut Hsva, alpha: Alpha, hex_code: &mut String) -> bool {
    let mut hsvag = HsvaGamma::from(*hsva);
    ui.vertical(|ui| {
        color_picker_hsvag_2d(ui, &mut hsvag, alpha, hex_code);
    });
    let new_hasva = Hsva::from(hsvag);
    if *hsva == new_hasva {
        false
    } else {
        *hsva = new_hasva;
        true
    }
}

pub fn color_picker_circle(
    ui: &mut Ui,
    color: &mut Color32,
    color_cache: &mut Vec<(Color32, f32)>,
    hex_code: &mut String,
) -> Response {
    let mut hsva: Hsva = (*color).into();
    let popup_id = ui.auto_id_with("popup");
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));
    let mut button_response = color_button(ui, hsva.into(), open, 25.0);
    if ui.style().explanation_tooltips {
        button_response = button_response.on_hover_text("Click to edit color");
    }

    if button_response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }

    const COLOR_SLIDER_WIDTH: f32 = 275.0;

    // TODO(emilk): make it easier to show a temporary popup that closes when you click outside it
    if ui.memory(|mem| mem.is_popup_open(popup_id)) {
        let area_response = Area::new(popup_id)
            .kind(UiKind::Picker)
            .order(Order::Foreground)
            .fixed_pos(button_response.rect.max)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().slider_width = COLOR_SLIDER_WIDTH;
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if color_picker_hsva_2d(
                            ui,
                            &mut hsva,
                            color_picker::Alpha::Opaque,
                            hex_code,
                        ) {
                            button_response.mark_changed();
                            *hex_code = Color32::from(hsva).to_hex().to_string();
                        }
                        ui.vertical(|ui| {
                            if add_button(ui, Color32::from(hsva), false).clicked() {
                                color_cache.push((Color32::from(hsva), 0.0));
                                *hex_code = Color32::from(hsva).to_hex().to_string();
                            }
                            for (color, size) in color_cache {
                                if color_button(ui, *color, false, *size).clicked() {
                                    hsva = Hsva::from(*color);
                                    *hex_code = color.to_hex().to_string();
                                };
                                if *size < 25.0 {
                                    *size += 1.5;
                                }
                            }
                        });
                    });
                });
            })
            .response;

        if !button_response.clicked()
            && (ui.input(|i| i.key_pressed(Key::Escape)) || area_response.clicked_elsewhere())
        {
            ui.memory_mut(|mem| mem.close_popup());
            hex_code.clear()
        }
        *color = Color32::from(hsva);
        return area_response;
    }
    return button_response;
}

// Widget class for ColorPicker (similar to Checkbox/Toggle pattern)
pub struct ColorPicker {
    pub color: Color32,
    pub color_cache: Vec<(Color32, f32)>,
    pub hex_code: String,
    size: f32,
}

impl ColorPicker {
    pub fn new(initial_color: Color32) -> Self {
        Self {
            color: initial_color,
            color_cache: Vec::new(),
            hex_code: String::new(),
            size: 25.0,
        }
    }

    pub fn set(&mut self, color: Color32) {
        self.color = color;
    }

    pub fn display(&mut self, ui: &mut Ui, label: &str) -> bool {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.add_space(10.0);
            self.show(ui)
        })
        .inner
    }

    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        let response = color_picker_circle(
            ui,
            &mut self.color,
            &mut self.color_cache,
            &mut self.hex_code,
        );
        if response.changed() {
            changed = true;
        }
        changed
    }
}
