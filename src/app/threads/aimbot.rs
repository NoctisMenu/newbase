
use std::collections::HashMap;

use crate::Player;


impl crate::App {
    pub fn start_aimbot_thread(&mut self) {
        let pbuf = self.player_buffer.clone();

        let mut reset_tracker: u64 = 0;
        self.nthread("aimbot thread", move || {
            
        })
        .unwrap();
    }
}
