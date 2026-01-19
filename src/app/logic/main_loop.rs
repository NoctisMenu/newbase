impl crate::App {
    pub fn main_loop(&mut self, painter: egui::Painter) {
        self.memory_aimbot();
        self.esp(painter.clone());
        self.exploits();
        self.discord_rpc();
    }
}
