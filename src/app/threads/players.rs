
use std::collections::HashMap;

use crate::Player;


impl crate::App {
    pub fn start_player_thread(&mut self) {
        let pbuf = self.player_buffer.clone();
        // Cache bone maps per entityptr to avoid rebuilding; works fairly well
        let mut skeleton_cache: HashMap<usize, Vec<usize>> = HashMap::new();

        //0x8 flags for checking validity
        //read frame count from uworld to check for scene change, if it is less than last frame, invalidate caches 0x680 u64
        let mut reset_tracker: u64 = 0;
        self.nthread("player thread", move || {
            reset_tracker += 1;
            if reset_tracker % 5000 == 0 {
                skeleton_cache.clear();
            }
            let mut write_size = 0;
            pbuf.write_batch_resize(60, |buf| {
                let player = Player::default();


                buf[write_size] = player;
                write_size += 1;
            });
            pbuf.shrink_size(write_size);
        })
        .unwrap();
    }
}
