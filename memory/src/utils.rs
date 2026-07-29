use windows::core::PCSTR;
use windows::Win32::Storage::FileSystem::QueryDosDeviceA;

pub fn device_path_to_dos_path(device_path: &str) -> Option<String> {
    let mut device_name = [0u8; 260];

    for letter in b'A'..=b'Z' {
        let drive = [letter, b':', 0];
        let len = unsafe { QueryDosDeviceA(PCSTR(drive.as_ptr()), Some(&mut device_name)) };

        if len > 0 {
            let name_end = device_name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(len as usize);
            let name = &device_name[..name_end];

            if let Ok(name_str) = std::str::from_utf8(name) {
                if device_path.len() >= name_str.len()
                    && device_path[..name_str.len()].eq_ignore_ascii_case(name_str)
                {
                    return Some(format!(
                        "{}:{}",
                        letter as char,
                        &device_path[name_str.len()..]
                    ));
                }
            }
        }
    }

    None
}

pub fn shuffle<T>(arr: &mut [T]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;

    let n = arr.len();
    if n <= 1 {
        return;
    }

    for i in (1..n).rev() {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (seed >> 33) as usize % (i + 1);
        arr.swap(i, j);
    }
}
