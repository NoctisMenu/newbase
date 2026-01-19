impl crate::App {
    pub fn discord_rpc(&mut self) {
        // Using the config! macro for cleaner, more efficient access
        let discord_enabled = crate::config!(self, "discord_presence");

        if discord_enabled && !self.discord.running() {
            self.discord.init();
        } else if !discord_enabled && self.discord.running() {
            self.discord.disable();
        }
        if discord_enabled {
            self.discord.set_details("noctis menu");
            self.discord.set_state("undetected - dev");
        }
    }
}
