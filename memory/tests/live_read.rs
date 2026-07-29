use ::memory::driver;
use ::memory::memory;

#[test]
fn live_memory_read_via_discord_exploit() {
    let target_name = "OxygenNotIncluded.exe";
    println!("[1] Looking for process: {}", target_name);
    let pid = driver::return_pid(target_name).expect("process not found");
    println!("    Found PID: {}", pid);

    println!("\n[1b] Checking if DiscordHook64 is loaded in target...");
    check_discord_hook(pid);

    println!("\n[2] Initializing exploit driver...");
    match memory::init_driver_diagnostic(pid) {
        Ok(()) => println!("    SUCCESS: driver initialized"),
        Err(step) => panic!("init_driver failed at: {}", step),
    }

    let base = memory::process_base();
    println!("\n[3] Process base: {:#x}", base);
    assert!(base != 0, "process base is null");

    println!("\n[4] Reading MZ header at base...");
    let bytes = memory::read_sized(base, 2).expect("read_sized failed");
    assert_eq!(&bytes[..], b"MZ", "expected MZ signature at process base");
    println!("    SUCCESS: read 'MZ' signature at {:#x}", base);

    println!("\n[5] Reading e_lfanew at base+0x3C...");
    let bytes = memory::read_sized(base + 0x3C, 4).expect("read e_lfanew failed");
    let e_lfanew = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    println!("    e_lfanew = {:#x}", e_lfanew);
    assert!(e_lfanew > 0 && e_lfanew < 0x1000, "invalid e_lfanew");

    println!("\n[6] Reading PE signature at base+{:#x}...", e_lfanew);
    let pe_bytes = memory::read_sized(base + e_lfanew, 4).expect("read PE sig failed");
    let sig = u32::from_le_bytes([pe_bytes[0], pe_bytes[1], pe_bytes[2], pe_bytes[3]]);
    assert_eq!(sig, 0x00004550, "expected PE signature");
    println!("    SUCCESS: read 'PE\\0\\0' signature");

    println!("\n[7] Reading 64-byte DOS header...");
    let bytes = memory::read_sized(base, 64).expect("read 64 bytes failed");
    assert_eq!(bytes.len(), 64);
    print!("    ");
    for (i, b) in bytes.iter().enumerate() {
        print!("{:02X} ", b);
        if (i + 1) % 16 == 0 {
            println!("\n    ");
        }
    }
    println!();

    println!("\n[8] Reading Machine field (base+e_lfanew+4)...");
    let bytes = memory::read_sized(base + e_lfanew + 4, 2).expect("read Machine failed");
    let machine = u16::from_le_bytes([bytes[0], bytes[1]]);
    println!("    Machine = {:#06x}", machine);
    assert_eq!(machine, 0x8664, "expected x86_64 machine type");
    println!("    SUCCESS: target is x86_64");

    println!("\n=== ALL LIVE READ TESTS PASSED ===");
}

fn check_discord_hook(pid: u32) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows::Win32::System::Memory::{
        VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, MEM_MAPPED, PAGE_READWRITE,
    };
    use windows::Win32::System::ProcessStatus::K32GetMappedFileNameA;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    unsafe {
        let h = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
            Ok(h) => h,
            Err(e) => {
                println!("    Cannot open process: {:?}", e);
                return;
            }
        };

        let mut address: *mut u8 = std::ptr::null_mut();
        let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
        let mut found_discord = false;
        let mut mapped_count = 0;

        loop {
            if VirtualQueryEx(
                h,
                Some(address as *const _),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            ) == 0
            {
                break;
            }

            if mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE && mbi.Protect.0 & 0x02 != 0 {
                let mut filename = [0u8; 1024];
                let len = K32GetMappedFileNameA(h, mbi.BaseAddress, &mut filename);
                if len > 0 {
                    let name = String::from_utf8_lossy(&filename[..len as usize]);
                    if name.contains("DiscordHook") {
                        println!("    FOUND: {} at {:#x}", name, address as usize);
                        found_discord = true;
                    }
                }
            }

            if mbi.State == MEM_COMMIT
                && mbi.Type == MEM_MAPPED
                && mbi.Protect == PAGE_READWRITE
                && mbi.RegionSize == 0x1000
            {
                mapped_count += 1;
            }

            address = address.add(mbi.RegionSize);
        }

        println!("    DiscordHook64 loaded: {}", found_discord);
        println!(
            "    0x1000 MEM_MAPPED RW regions in target: {}",
            mapped_count
        );

        let _ = CloseHandle(h);
    }
}
