use std::collections::BTreeMap;
use std::sync::{LazyLock, OnceLock};

use anyhow::Result;
use log::{debug, error};
use pelite::pattern;
use pelite::pattern::{Atom, save_len};
use pelite::pe64::{Pe, PeView, Rva};
use phf::{Map, phf_map};

pub mod client_dll;
pub mod offsets;

static CLIENT_BASE: LazyLock<usize> = LazyLock::new(|| {
    let pid = secure_process_memory::return_pid("deadlock.exe").unwrap();
    let process = secure_process_memory::Process::new(pid).unwrap();
    process
        .get_module_base_address("client.dll")
        .unwrap()
        .unwrap() as usize
});

static OFFSETS: OnceLock<[usize; 2]> = OnceLock::new();

pub fn client_base() -> usize {
    *CLIENT_BASE
}

pub fn resolved_offsets() -> &'static [usize; 2] {
    OFFSETS.get_or_init(|| {
        //let offsets = scan_offsets().unwrap();
        //let val = offsets.get("client.dll").unwrap();
        [
            offsets::cs2_dumper::offsets::client_dll::dwEntityList,
            offsets::cs2_dumper::offsets::client_dll::dwViewMatrix,
        ]
    })
}
