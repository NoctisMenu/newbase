use std::mem;

#[test]
fn wparam_struct_layout_matches_cpp() {
    assert_eq!(mem::size_of::<memory::memory::WParamLayout>(), 0x50);
    assert_eq!(mem::offset_of!(memory::memory::WParamLayout, memcpy), 0x00);
    assert_eq!(mem::offset_of!(memory::memory::WParamLayout, arg2), 0x08);
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, function_ptr),
        0x10
    );
    assert_eq!(mem::offset_of!(memory::memory::WParamLayout, arg1), 0x18);
    assert_eq!(mem::offset_of!(memory::memory::WParamLayout, lock), 0x20);
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, dst_length),
        0x28
    );
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, dst_max_length),
        0x2A
    );
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, dst_buffer),
        0x30
    );
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, src_length),
        0x38
    );
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, src_max_length),
        0x3A
    );
    assert_eq!(
        mem::offset_of!(memory::memory::WParamLayout, src_buffer),
        0x40
    );
    assert_eq!(mem::offset_of!(memory::memory::WParamLayout, val), 0x48);
}

#[test]
fn pattern_scan_finds_known_ntdll_export() {
    use windows::core::PCSTR;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

    unsafe {
        let nt = GetModuleHandleA(PCSTR(b"ntdll.dll\0".as_ptr())).unwrap();
        let rtl_copy = GetProcAddress(nt, PCSTR(b"RtlCopyMemory\0".as_ptr())).unwrap();

        let first_bytes = std::slice::from_raw_parts(rtl_copy as *const u8, 8);
        let pattern = first_bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        let found = memory::memory::pattern_scan_module_public(nt.0 as *const u8, &pattern);
        assert!(
            found.is_some(),
            "pattern scan should find RtlCopyMemory prologue"
        );
        assert_eq!(found.unwrap(), rtl_copy as usize);
    }
}

#[test]
fn pattern_scan_with_wildcards() {
    use windows::core::PCSTR;
    use windows::Win32::System::LibraryLoader::GetModuleHandleA;

    unsafe {
        let nt = GetModuleHandleA(PCSTR(b"ntdll.dll\0".as_ptr())).unwrap();
        let result = memory::memory::pattern_scan_module_public(
            nt.0 as *const u8,
            "48 89 5C 24 ?? 57 48 83 EC",
        );
        assert!(
            result.is_some(),
            "wildcard pattern should match somewhere in ntdll"
        );
    }
}

#[test]
fn pe_mapper_loads_ntdll_from_disk() {
    let mapper = memory::pe_mapper::PEMemoryMapper::new("C:\\Windows\\System32\\ntdll.dll");
    assert!(mapper.is_some(), "should parse ntdll.dll from disk");

    let mapper = mapper.unwrap();
    assert!(mapper.base_address() > 0);
    assert!(mapper.memory_size() > 0x1000);

    let mz = mapper.read_from_va(mapper.base_address(), 2).unwrap();
    assert_eq!(mz, b"MZ");
}

#[test]
fn pe_mapper_sigscan_finds_pattern() {
    let mapper = memory::pe_mapper::PEMemoryMapper::new("C:\\Windows\\System32\\ntdll.dll");
    assert!(mapper.is_some());
    let mapper = mapper.unwrap();

    let result = mapper.sigscan("4D 5A");
    assert!(result.is_some(), "MZ header should be findable via sigscan");
}

#[test]
fn return_pid_finds_explorer() {
    let pid = memory::driver::return_pid("explorer.exe");
    assert!(pid.is_some(), "explorer.exe should be running");
    assert!(pid.unwrap() > 0);
}

#[test]
fn return_pid_returns_none_for_fake_process() {
    let pid = memory::driver::return_pid("nonexistent_process_xyz123.exe");
    assert!(pid.is_none());
}

#[test]
fn read_sized_fails_when_not_initialized() {
    let result = memory::memory::read_sized(0x1000, 8);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        memory::types::MemoryError::NotInitialized
    );
}

#[test]
fn writef_fails_when_not_initialized() {
    let result = memory::memory::writef(0x1000, 0u64);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        memory::types::MemoryError::NotInitialized
    );
}

#[test]
fn process_base_is_zero_before_init() {
    assert_eq!(memory::memory::process_base(), 0);
}

#[test]
fn device_path_to_dos_path_resolves_system_drive() {
    let result = memory::utils::device_path_to_dos_path(
        "\\Device\\HarddiskVolume1\\Windows\\System32\\ntdll.dll",
    );
    if result.is_some() {
        let path = result.unwrap();
        assert!(path.ends_with("\\Windows\\System32\\ntdll.dll"));
        assert!(path.len() >= 3 && path.as_bytes()[1] == b':');
    }
}

#[test]
fn find_rop_gadgets_all_modules() {
    use windows::core::PCSTR;
    use windows::Win32::System::LibraryLoader::LoadLibraryA;

    unsafe {
        let modules: [(&str, &[u8]); 6] = [
            ("ntdll", b"ntdll.dll\0"),
            ("kernelbase", b"kernelbase.dll\0"),
            ("kernel32", b"kernel32.dll\0"),
            ("user32", b"user32.dll\0"),
            ("shell32", b"shell32.dll\0"),
            ("uxtheme", b"uxtheme.dll\0"),
        ];

        for (mod_name, mod_path) in &modules {
            let h = LoadLibraryA(PCSTR(mod_path.as_ptr()));
            if h.is_err() {
                continue;
            }
            let h = h.unwrap();
            let base = h.0 as *const u8;
            let e_lfanew = *(base.add(0x3C) as *const u32) as usize;
            let nt_hdr = base.add(e_lfanew);
            let size = *(nt_hdr.add(0x18 + 0x38) as *const u32) as usize;
            let code = std::slice::from_raw_parts(base, size);

            // Search for trampoline pattern: loads fn ptr from [reg+0], sets up args from
            // same struct (offsets 0x28/0x38), then calls (either call rax or call [rip+disp32])
            //
            // Key sequences to find:
            // A) mov rax,[rcx]; ... lea rcx,[rcx+0x28] or lea rdx,[rcx+0x38] ... call
            // B) mov rax,[rdx]; ... lea rcx,[rdx+0x28] or lea rdx,[rdx+0x38] ... call
            // C) mov rax,[rbx]; ... (rbx was set from rcx/rdx earlier)

            let mut found_any = false;

            for i in 0..size.saturating_sub(30) {
                // 48 8B 01 = mov rax,[rcx]  |  48 8B 02 = mov rax,[rdx]  |  48 8B 03 = mov rax,[rbx]
                let is_load = code[i] == 0x48
                    && code[i + 1] == 0x8B
                    && code[i + 2] >= 0x01
                    && code[i + 2] <= 0x03;
                if !is_load {
                    continue;
                }
                let src_reg = match code[i + 2] {
                    0x01 => "rcx",
                    0x02 => "rdx",
                    0x03 => "rbx",
                    _ => continue,
                };

                // Within next 24 bytes, look for BOTH:
                // 1) A lea/add with offset 0x28 or 0x38 (arg setup from struct)
                // 2) A call (FF D0 = call rax, FF 15 = call [rip+disp32], FF D3 = call rbx)
                let mut has_arg_setup = false;
                let mut has_call = false;
                let mut call_off = 0usize;

                for j in 3..28.min(size - i - 2) {
                    let off = i + j;
                    // lea rcx,[reg+0x28]: 48 8D 4x 28 or 48 8D 8x 28 00 00 00
                    if code[off] == 0x48 && code[off + 1] == 0x8D {
                        let modrm = code[off + 2];
                        if (modrm & 0xC7) == 0x41 && code[off + 3] == 0x28 {
                            has_arg_setup = true;
                        } // lea rcx,[rcx+0x28]
                        if (modrm & 0xC7) == 0x42 && code[off + 3] == 0x28 {
                            has_arg_setup = true;
                        } // lea rcx,[rdx+0x28]
                        if (modrm & 0xC7) == 0x41 && code[off + 3] == 0x38 {
                            has_arg_setup = true;
                        } // lea rcx,[rcx+0x38]
                        if (modrm & 0xC7) == 0x42 && code[off + 3] == 0x38 {
                            has_arg_setup = true;
                        } // lea rcx,[rdx+0x38]
                        if (modrm & 0xC7) == 0x51 && code[off + 3] == 0x28 {
                            has_arg_setup = true;
                        } // lea rdx,[rcx+0x28]
                        if (modrm & 0xC7) == 0x52 && code[off + 3] == 0x28 {
                            has_arg_setup = true;
                        } // lea rdx,[rdx+0x28]
                        if (modrm & 0xC7) == 0x51 && code[off + 3] == 0x38 {
                            has_arg_setup = true;
                        } // lea rdx,[rcx+0x38]
                        if (modrm & 0xC7) == 0x52 && code[off + 3] == 0x38 {
                            has_arg_setup = true;
                        } // lea rdx,[rdx+0x38]
                    }
                    // add rcx,0x28: 48 83 C1 28 | add rdx,0x28: 48 83 C2 28
                    if code[off] == 0x48 && code[off + 1] == 0x83 {
                        if code[off + 2] == 0xC1 && code[off + 3] == 0x28 {
                            has_arg_setup = true;
                        }
                        if code[off + 2] == 0xC2 && code[off + 3] == 0x28 {
                            has_arg_setup = true;
                        }
                        if code[off + 2] == 0xC1 && code[off + 3] == 0x38 {
                            has_arg_setup = true;
                        }
                        if code[off + 2] == 0xC2 && code[off + 3] == 0x38 {
                            has_arg_setup = true;
                        }
                    }
                    // call rax (FF D0), call rbx (FF D3), call [rip+disp32] (FF 15)
                    if code[off] == 0xFF && (code[off + 1] == 0xD0 || code[off + 1] == 0xD3) {
                        has_call = true;
                        call_off = j;
                    }
                    if code[off] == 0xFF && code[off + 1] == 0x15 {
                        has_call = true;
                        call_off = j;
                    }
                }

                if has_arg_setup && has_call {
                    if !found_any {
                        eprintln!(
                            "\n=== [{}] Trampoline candidates (load fn ptr + arg setup + call) ===",
                            mod_name
                        );
                        found_any = true;
                    }
                    let ctx: String = code[i..(i + 32).min(size)]
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!(
                        "  +{:#x}: mov rax,[{}] ... call @+{}  {}",
                        i, src_reg, call_off, ctx
                    );
                }
            }

            // Also search for: mov rbx,rcx/rdx; ... mov rax,[rbx]; ... call
            // (function saves struct ptr to rbx, then dispatches)
            for i in 0..size.saturating_sub(30) {
                // 48 8B DA = mov rbx,rdx  |  48 8B D9 = mov rbx,rcx
                let is_save = code[i] == 0x48
                    && code[i + 1] == 0x8B
                    && (code[i + 2] == 0xDA || code[i + 2] == 0xD9);
                if !is_save {
                    continue;
                }
                let src = if code[i + 2] == 0xDA { "rdx" } else { "rcx" };

                // Within next 30 bytes: mov rax,[rbx+0] (48 8B 03) or mov rax,[rbx+disp8] (48 8B 43 xx)
                // followed by arg setup and call
                let mut has_fn_load = false;
                let mut has_arg = false;
                let mut has_call = false;
                let mut fn_disp = 0u8;

                for j in 3..30.min(size - i - 2) {
                    let off = i + j;
                    if code[off] == 0x48 && code[off + 1] == 0x8B && code[off + 2] == 0x03 {
                        has_fn_load = true;
                        fn_disp = 0;
                    }
                    if code[off] == 0x48
                        && code[off + 1] == 0x8B
                        && code[off + 2] == 0x43
                        && code[off + 3] < 0x40
                    {
                        has_fn_load = true;
                        fn_disp = code[off + 3];
                    }
                    // lea rcx,[rbx+0x28]: 48 8D 4B 28  |  lea rdx,[rbx+0x38]: 48 8D 53 38
                    if code[off] == 0x48 && code[off + 1] == 0x8D {
                        if code[off + 2] == 0x4B && (code[off + 3] == 0x28 || code[off + 3] == 0x38)
                        {
                            has_arg = true;
                        }
                        if code[off + 2] == 0x53 && (code[off + 3] == 0x28 || code[off + 3] == 0x38)
                        {
                            has_arg = true;
                        }
                    }
                    if code[off] == 0xFF
                        && (code[off + 1] == 0xD0 || code[off + 1] == 0xD3 || code[off + 1] == 0x15)
                    {
                        has_call = true;
                    }
                }

                if has_fn_load && has_arg && has_call {
                    if !found_any {
                        eprintln!(
                            "\n=== [{}] Trampoline candidates (save rbx + load + arg + call) ===",
                            mod_name
                        );
                        found_any = true;
                    }
                    let ctx: String = code[i..(i + 40).min(size)]
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!(
                        "  +{:#x}: mov rbx,{}; mov rax,[rbx+{:#x}]; ... call  {}",
                        i, src, fn_disp, ctx
                    );
                }
            }

            if !found_any {
                eprintln!("\n=== [{}] No trampoline candidates found ===", mod_name);
            }
        }
    }
}
