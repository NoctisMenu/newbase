use egui::Color32;

#[derive(Clone, Debug)]
pub struct Colors {
    pub background_off: Color32,
    pub background_on: Color32,
    pub handle_color: Color32,
}

impl Colors {
    pub fn new(background_off: Color32, background_on: Color32, handle_color: Color32) -> Self {
        Self {
            background_off,
            background_on,
            handle_color,
        }
    }
    pub fn set_on_color(mut self, background_on: Color32) -> Self {
        self.background_on = background_on;
        self
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            background_off: Color32::from_rgb(25, 25, 25),
            background_on: Color32::from_rgb(144, 238, 144),
            handle_color: Color32::from_rgb(200, 200, 200),
        }
    }
}
