impl crate::App {
    pub fn spawn_all_threads(&mut self) {
        self.start_player_thread();
        self.start_aimbot_thread();
        self.start_cache_thread();
    }
}
