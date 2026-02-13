
use std::collections::HashMap;

impl crate::App {
    pub fn start_cache_thread(&mut self) {
        self.nthread("cache thread", move || {
            
        })
        .unwrap();
    }
}
