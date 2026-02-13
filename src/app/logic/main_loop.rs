use newoverlay::imgui::{Ui, DrawListMut};

impl crate::App {
    pub fn main_loop(&mut self, ui: &Ui, dl: &DrawListMut) {
        self.esp(dl);
        self.exploits();
    }
}
