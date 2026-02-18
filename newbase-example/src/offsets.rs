use std::collections::BTreeMap;
use std::sync::{LazyLock, OnceLock};

use anyhow::Result;
use log::{debug, error};
use pelite::pattern;
use pelite::pattern::{Atom, save_len};
use pelite::pe64::{Pe, PeView, Rva};
use phf::{Map, phf_map};

pub type OffsetMap = BTreeMap<String, BTreeMap<String, Rva>>;

macro_rules! pattern_map {
    ($($module:ident => {
        $($name:expr => $pattern:expr $(=> $callback:expr)?),+ $(,)?
    }),+ $(,)?) => {
        $(
            mod $module {
                use super::*;

                pub(super) const PATTERNS: Map<
                    &'static str,
                    (
                        &'static [Atom],
                        Option<fn(&PeView, &mut BTreeMap<String, Rva>, Rva)>,
                    ),
                > = phf_map! {
                    $($name => ($pattern, $($callback)?)),+
                };

                pub fn offsets(view: PeView<'_>) -> BTreeMap<String, Rva> {
                    let mut map = BTreeMap::new();

                    for (&name, (pat, callback)) in &PATTERNS {
                        let mut save = vec![0; save_len(pat)];

                        if !view.scanner().finds_code(pat, &mut save) {
                            error!("outdated pattern: {}", name);
                            continue;
                        }

                        let rva = save[1];
                        map.insert(name.to_string(), rva);

                        if let Some(callback) = callback {
                            callback(&view, &mut map, rva);
                        }
                    }

                    for (name, value) in &map {
                        debug!(
                            "found offset: {} at {:#X} ({}.dll + {:#X})",
                            name,
                            *value as u64 + view.optional_header().ImageBase,
                            stringify!($module),
                            value
                        );
                    }

                    map
                }
            }
        )+
    };
}

pattern_map! {
    client => {
        "dwEntityList" => pattern!("488b0d${'} 48897c24? 8bfac1eb") => None,
        "dwViewMatrix" => pattern!("488d0d${'} 48c1e006") => None,
    }
}

pub fn scan_offsets() -> Result<OffsetMap> {
    let mut map = BTreeMap::new();
    let modules: [(&str, fn(PeView) -> BTreeMap<String, u32>); 1] = [("client.dll", client::offsets)];

    let pid = secure_process_memory::return_pid("deadlock.exe").unwrap();
    let process = secure_process_memory::Process::new(pid).unwrap();
    let mut found = false;

    for (module_name, offsets) in &modules {
        if let Some(module) = dbg!(process.get_module_base_address(module_name))? {
            found = true;
            let buf = memory::memory::read_sized(module as usize, 0x2ECDD00).unwrap();
            let view = PeView::from_bytes(&buf)?;
            map.insert(module_name.to_string(), offsets(view));
        }
    }

    if found {
        Ok(map)
    } else {
        Err(anyhow::Error::msg("failed to find"))
    }
}

static CLIENT_BASE: LazyLock<usize> = LazyLock::new(|| {
    let pid = secure_process_memory::return_pid("deadlock.exe").unwrap();
    let process = secure_process_memory::Process::new(pid).unwrap();
    process.get_module_base_address("client.dll").unwrap().unwrap() as usize
});

static OFFSETS: OnceLock<[usize; 2]> = OnceLock::new();

pub fn client_base() -> usize {
    *CLIENT_BASE
}

pub fn resolved_offsets() -> &'static [usize; 2] {
    OFFSETS.get_or_init(|| {
        let offsets = scan_offsets().unwrap();
        let val = offsets.get("client.dll").unwrap();
        [
            *val.get("dwEntityList").unwrap() as usize,
            *val.get("dwViewMatrix").unwrap() as usize,
        ]
    })
}
