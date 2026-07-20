//! Reversible function detours installed in a remote 64-bit process.

use std::{
    ffi::c_void,
    mem::{ManuallyDrop, size_of},
};

use iced_x86::{
    BlockEncoder, BlockEncoderOptions, Code, Decoder, DecoderOptions, IcedError, Instruction,
    InstructionBlock,
};
use thiserror::Error;
use windows::{
    Win32::System::{
        Diagnostics::Debug::FlushInstructionCache,
        Memory::{
            MEM_COMMIT, MEM_FREE, MEM_RELEASE, MEM_RESERVE, MEMORY_BASIC_INFORMATION,
            PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
            VirtualAllocEx, VirtualFreeEx, VirtualProtectEx, VirtualQueryEx,
        },
        SystemInformation::{GetSystemInfo, SYSTEM_INFO},
    },
    core::Error as WindowsError,
};

use crate::RemoteProcess;

const RELATIVE_JUMP_SIZE: usize = 5;
const ABSOLUTE_JUMP_SIZE: usize = 14;
const MAX_PROLOGUE_BYTES: usize = 64;
const HOOK_ALLOCATION_SIZE: usize = 0x1000;
const RELATIVE_RANGE: usize = i32::MAX as usize;

/// How an installed remote hook obtains its destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteHookKind {
    /// The function entry jumps to code that already exists in the target process.
    Detour,
    /// The function entry jumps to newly allocated position-independent code,
    /// which then continues through the relocated original instructions.
    InjectedCode,
}

/// Errors produced while constructing, enabling, or removing a remote hook.
#[derive(Debug, Error)]
pub enum RemoteHookError {
    #[error(
        "the newbase memory driver is not initialized; create and run the App before installing hooks"
    )]
    DriverNotInitialized,
    #[error(transparent)]
    RemoteProcess(#[from] crate::RemoteCallError),
    #[error("hook target address cannot be null")]
    NullTarget,
    #[error("hook destination address cannot be null")]
    NullDestination,
    #[error("injected hook code cannot be empty")]
    EmptyHookCode,
    #[error("failed to decode an instruction at remote address {address:#x}")]
    InvalidInstruction { address: usize },
    #[error("failed to relocate the target prologue: {0}")]
    Relocation(#[from] IcedError),
    #[error("generated hook code needs {required} bytes, but its allocation has {available}")]
    AllocationTooSmall { required: usize, available: usize },
    #[error("remote code at {address:#x} changed while the hook was being updated")]
    CodeChanged { address: usize },
    #[error("Windows operation '{operation}' failed: {source}")]
    Windows {
        operation: &'static str,
        source: WindowsError,
    },
    #[error("driver memory operation '{operation}' failed: {error:?}")]
    DriverMemory {
        operation: &'static str,
        error: memory::types::MemoryError,
    },
}

/// A reversible hook installed at a function entry in a remote process.
///
/// The hook owns an executable allocation containing a relocated trampoline and,
/// for [`RemoteHook::install_code`], the supplied hook prelude. Dropping the hook
/// restores the original entry bytes before releasing that allocation.
pub struct RemoteHook {
    process: RemoteProcess,
    target: usize,
    destination: usize,
    trampoline: usize,
    original_bytes: Vec<u8>,
    patch_bytes: Vec<u8>,
    allocation: Option<HookAllocation>,
    enabled: bool,
    kind: RemoteHookKind,
}

impl RemoteHook {
    /// Redirect a remote function to existing code in the same process.
    ///
    /// A trampoline containing the overwritten instructions is allocated in the
    /// target process and can be obtained with [`RemoteHook::trampoline_address`].
    /// Newbase's global memory driver must already be initialized for this same
    /// process; application code should normally use
    /// [`crate::App::install_remote_detour`] after startup.
    ///
    /// # Safety
    ///
    /// `target` and `destination` must be valid x64 code addresses in `process`.
    /// The caller must ensure no target thread executes the patched bytes while
    /// they are being changed, and must keep the hook alive while any thread can
    /// execute its trampoline.
    pub unsafe fn install_detour(
        process: &RemoteProcess,
        target: usize,
        destination: usize,
    ) -> Result<Self, RemoteHookError> {
        ensure_driver_initialized()?;
        if target == 0 {
            return Err(RemoteHookError::NullTarget);
        }
        if destination == 0 {
            return Err(RemoteHookError::NullDestination);
        }

        let allocation = HookAllocation::allocate_near(process.clone(), target)?;
        let target_jump = encode_jump(target, destination);
        let (instructions, original_bytes) = decode_prologue(process, target, target_jump.len())?;
        let trampoline = allocation.address();
        let trampoline_code =
            build_trampoline(&instructions, trampoline, target + original_bytes.len())?;
        allocation.write_at(0, &trampoline_code)?;
        allocation.make_executable()?;

        let patch_bytes = padded_patch(target_jump, original_bytes.len());
        let allocation = install_patch(process, target, &original_bytes, &patch_bytes, allocation)?;

        Ok(Self {
            process: process.clone(),
            target,
            destination,
            trampoline,
            original_bytes,
            patch_bytes,
            allocation: Some(allocation),
            enabled: true,
            kind: RemoteHookKind::Detour,
        })
    }

    /// Inject a position-independent code prelude and run it before the original function.
    ///
    /// The prelude is copied to newly allocated executable memory. A jump to the
    /// relocated original instructions is appended automatically. The prelude
    /// must preserve any argument registers, stack state, and nonvolatile state
    /// that the original function still needs.
    /// Newbase's global memory driver must already target `process`; application
    /// code should normally use [`crate::App::install_remote_code_hook`].
    ///
    /// # Safety
    ///
    /// `target` must be a valid x64 function entry and `code` must be valid,
    /// position-independent x64 machine code that can safely fall through to the
    /// generated continuation jump. The caller must synchronize target threads
    /// while enabling, disabling, or dropping the hook.
    pub unsafe fn install_code(
        process: &RemoteProcess,
        target: usize,
        code: &[u8],
    ) -> Result<Self, RemoteHookError> {
        ensure_driver_initialized()?;
        if target == 0 {
            return Err(RemoteHookError::NullTarget);
        }
        if code.is_empty() {
            return Err(RemoteHookError::EmptyHookCode);
        }
        let minimum_required = code.len().saturating_add(RELATIVE_JUMP_SIZE);
        if minimum_required > HOOK_ALLOCATION_SIZE {
            return Err(RemoteHookError::AllocationTooSmall {
                required: minimum_required,
                available: HOOK_ALLOCATION_SIZE,
            });
        }

        let allocation = HookAllocation::allocate_near(process.clone(), target)?;
        let destination = allocation.address();
        let target_jump = encode_jump(target, destination);
        let (instructions, original_bytes) = decode_prologue(process, target, target_jump.len())?;

        // Both blocks share one allocation, so this continuation is always rel32.
        let trampoline_offset = align_up(code.len() + RELATIVE_JUMP_SIZE, 16);
        let trampoline = destination + trampoline_offset;
        let trampoline_code =
            build_trampoline(&instructions, trampoline, target + original_bytes.len())?;
        let continuation = encode_jump(destination + code.len(), trampoline);
        let required = trampoline_offset + trampoline_code.len();
        if required > allocation.size {
            return Err(RemoteHookError::AllocationTooSmall {
                required,
                available: allocation.size,
            });
        }

        allocation.write_at(0, code)?;
        allocation.write_at(code.len(), &continuation)?;
        allocation.write_at(trampoline_offset, &trampoline_code)?;
        allocation.make_executable()?;

        let patch_bytes = padded_patch(target_jump, original_bytes.len());
        let allocation = install_patch(process, target, &original_bytes, &patch_bytes, allocation)?;

        Ok(Self {
            process: process.clone(),
            target,
            destination,
            trampoline,
            original_bytes,
            patch_bytes,
            allocation: Some(allocation),
            enabled: true,
            kind: RemoteHookKind::InjectedCode,
        })
    }

    pub fn kind(&self) -> RemoteHookKind {
        self.kind
    }

    pub fn target_address(&self) -> usize {
        self.target
    }

    pub fn destination_address(&self) -> usize {
        self.destination
    }

    /// Address of the relocated original prologue followed by a jump back.
    pub fn trampoline_address(&self) -> usize {
        self.trampoline
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Restore the original entry bytes while retaining the trampoline allocation.
    ///
    /// # Safety
    ///
    /// The caller must ensure no target thread is executing the patched entry.
    pub unsafe fn disable(&mut self) -> Result<(), RemoteHookError> {
        if !self.enabled {
            return Ok(());
        }
        let result = replace_code_checked(
            &self.process,
            self.target,
            &self.patch_bytes,
            &self.original_bytes,
        );
        if result.is_ok() || code_matches(&self.process, self.target, &self.original_bytes) {
            self.enabled = false;
        }
        result
    }

    /// Reapply a disabled hook.
    ///
    /// # Safety
    ///
    /// The caller must ensure no target thread is executing the patched entry.
    pub unsafe fn enable(&mut self) -> Result<(), RemoteHookError> {
        if self.enabled {
            return Ok(());
        }
        let result = replace_code_checked(
            &self.process,
            self.target,
            &self.original_bytes,
            &self.patch_bytes,
        );
        if result.is_ok() || code_matches(&self.process, self.target, &self.patch_bytes) {
            self.enabled = true;
        }
        result
    }

    /// Restore the original function and release all remote hook memory.
    ///
    /// # Safety
    ///
    /// The caller must ensure no target thread is executing the hook or trampoline.
    pub unsafe fn remove(mut self) -> Result<(), RemoteHookError> {
        // SAFETY: The caller accepted `disable`'s synchronization contract.
        unsafe { self.disable()? };
        drop(self.allocation.take());
        Ok(())
    }
}

impl Drop for RemoteHook {
    fn drop(&mut self) {
        if self.enabled {
            // SAFETY: Installation requires the owner to uphold synchronization
            // through drop. If restoration fails, leak the allocation so an entry
            // that still points at it never becomes a dangling jump.
            if unsafe { self.disable() }.is_err()
                && let Some(allocation) = self.allocation.take()
            {
                allocation.leak();
            }
        }
    }
}

struct HookAllocation {
    process: RemoteProcess,
    address: *mut c_void,
    size: usize,
}

// The pointer names memory in another process and is never locally dereferenced.
unsafe impl Send for HookAllocation {}
unsafe impl Sync for HookAllocation {}

impl HookAllocation {
    fn allocate_near(process: RemoteProcess, target: usize) -> Result<Self, RemoteHookError> {
        let address = find_and_allocate_near(&process, target, HOOK_ALLOCATION_SIZE)
            .or_else(|| allocate_anywhere(&process, HOOK_ALLOCATION_SIZE));
        let Some(address) = address else {
            return Err(RemoteHookError::Windows {
                operation: "VirtualAllocEx(hook)",
                source: WindowsError::from_thread(),
            });
        };
        Ok(Self {
            process,
            address,
            size: HOOK_ALLOCATION_SIZE,
        })
    }

    fn address(&self) -> usize {
        self.address as usize
    }

    fn write_at(&self, offset: usize, bytes: &[u8]) -> Result<(), RemoteHookError> {
        let end = offset.saturating_add(bytes.len());
        if end > self.size {
            return Err(RemoteHookError::AllocationTooSmall {
                required: end,
                available: self.size,
            });
        }
        write_memory(
            &self.process,
            self.address().saturating_add(offset),
            bytes,
            "write hook allocation",
        )
    }

    fn make_executable(&self) -> Result<(), RemoteHookError> {
        let mut old = PAGE_PROTECTION_FLAGS::default();
        // SAFETY: The allocation is live and `old` points to writable local memory.
        unsafe {
            VirtualProtectEx(
                self.process.raw_handle(),
                self.address.cast_const(),
                self.size,
                PAGE_EXECUTE_READ,
                &mut old,
            )
        }
        .map_err(|source| RemoteHookError::Windows {
            operation: "VirtualProtectEx(hook)",
            source,
        })?;
        flush_instructions(&self.process, self.address(), self.size)
    }

    fn leak(self) {
        let _ = ManuallyDrop::new(self);
    }
}

impl Drop for HookAllocation {
    fn drop(&mut self) {
        // SAFETY: This object uniquely owns the remote MEM_RESERVE allocation.
        unsafe {
            let _ = VirtualFreeEx(self.process.raw_handle(), self.address, 0, MEM_RELEASE);
        }
    }
}

fn decode_prologue(
    process: &RemoteProcess,
    target: usize,
    minimum_size: usize,
) -> Result<(Vec<Instruction>, Vec<u8>), RemoteHookError> {
    let bytes = read_memory_prefix(process, target, MAX_PROLOGUE_BYTES)?;
    let mut decoder = Decoder::with_ip(64, &bytes, target as u64, DecoderOptions::NONE);
    let mut instructions = Vec::new();
    let mut decoded_size = 0;
    while decoded_size < minimum_size {
        if !decoder.can_decode() {
            return Err(RemoteHookError::InvalidInstruction {
                address: target + decoded_size,
            });
        }
        let instruction = decoder.decode();
        if instruction.is_invalid() || instruction.len() == 0 {
            return Err(RemoteHookError::InvalidInstruction {
                address: instruction.ip() as usize,
            });
        }
        decoded_size += instruction.len();
        instructions.push(instruction);
    }
    bytes
        .get(..decoded_size)
        .map(|original| (instructions, original.to_vec()))
        .ok_or(RemoteHookError::InvalidInstruction {
            address: target + decoded_size,
        })
}

fn build_trampoline(
    instructions: &[Instruction],
    trampoline: usize,
    return_address: usize,
) -> Result<Vec<u8>, RemoteHookError> {
    let mut relocated_instructions = instructions.to_vec();
    relocated_instructions.push(Instruction::with_branch(
        Code::Jmp_rel32_64,
        return_address as u64,
    )?);
    Ok(BlockEncoder::encode(
        64,
        InstructionBlock::new(&relocated_instructions, trampoline as u64),
        BlockEncoderOptions::NONE,
    )?
    .code_buffer)
}

fn encode_jump(instruction_address: usize, destination: usize) -> Vec<u8> {
    if let Some(displacement) = relative_displacement(instruction_address, destination) {
        let mut jump = Vec::with_capacity(RELATIVE_JUMP_SIZE);
        jump.push(0xe9);
        jump.extend_from_slice(&displacement.to_le_bytes());
        jump
    } else {
        let mut jump = Vec::with_capacity(ABSOLUTE_JUMP_SIZE);
        jump.extend_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
        jump.extend_from_slice(&(destination as u64).to_le_bytes());
        jump
    }
}

fn relative_displacement(instruction_address: usize, destination: usize) -> Option<i32> {
    let next_instruction = instruction_address.checked_add(RELATIVE_JUMP_SIZE)?;
    let displacement = destination as i128 - next_instruction as i128;
    i32::try_from(displacement).ok()
}

fn padded_patch(mut jump: Vec<u8>, overwritten_size: usize) -> Vec<u8> {
    jump.resize(overwritten_size, 0x90);
    jump
}

fn install_patch(
    process: &RemoteProcess,
    target: usize,
    original: &[u8],
    patch: &[u8],
    allocation: HookAllocation,
) -> Result<HookAllocation, RemoteHookError> {
    if read_memory(process, target, original.len())? != original {
        return Err(RemoteHookError::CodeChanged { address: target });
    }
    match replace_code(process, target, patch) {
        Ok(()) => Ok(allocation),
        Err(error) => {
            if replace_code(process, target, original).is_err() {
                // The target may contain a partial jump into this allocation.
                // Keeping it allocated is safer than freeing a live destination.
                allocation.leak();
            }
            Err(error)
        }
    }
}

fn replace_code_checked(
    process: &RemoteProcess,
    address: usize,
    expected: &[u8],
    replacement: &[u8],
) -> Result<(), RemoteHookError> {
    if read_memory(process, address, expected.len())? != expected {
        return Err(RemoteHookError::CodeChanged { address });
    }
    replace_code(process, address, replacement)
}

fn code_matches(process: &RemoteProcess, address: usize, expected: &[u8]) -> bool {
    read_memory(process, address, expected.len()).is_ok_and(|bytes| bytes == expected)
}

fn replace_code(
    process: &RemoteProcess,
    address: usize,
    replacement: &[u8],
) -> Result<(), RemoteHookError> {
    let mut old = PAGE_PROTECTION_FLAGS::default();
    // SAFETY: The caller provided a live code range and writable output storage.
    unsafe {
        VirtualProtectEx(
            process.raw_handle(),
            address as *const c_void,
            replacement.len(),
            PAGE_EXECUTE_READWRITE,
            &mut old,
        )
    }
    .map_err(|source| RemoteHookError::Windows {
        operation: "VirtualProtectEx(target writable)",
        source,
    })?;

    let write_result = write_memory(process, address, replacement, "write target patch");
    let flush_result = flush_instructions(process, address, replacement.len());
    let mut ignored = PAGE_PROTECTION_FLAGS::default();
    // SAFETY: This restores protection on the same live range.
    let restore_result = unsafe {
        VirtualProtectEx(
            process.raw_handle(),
            address as *const c_void,
            replacement.len(),
            old,
            &mut ignored,
        )
    }
    .map_err(|source| RemoteHookError::Windows {
        operation: "VirtualProtectEx(target restore)",
        source,
    });

    write_result?;
    flush_result?;
    restore_result
}

fn read_memory(
    process: &RemoteProcess,
    address: usize,
    size: usize,
) -> Result<Vec<u8>, RemoteHookError> {
    let _ = process;
    crate::read_sized(address, size).map_err(|error| RemoteHookError::DriverMemory {
        operation: "read remote memory",
        error,
    })
}

fn read_memory_prefix(
    process: &RemoteProcess,
    address: usize,
    maximum_size: usize,
) -> Result<Vec<u8>, RemoteHookError> {
    let mut bytes = Vec::with_capacity(maximum_size);
    while bytes.len() < maximum_size {
        let cursor = address.saturating_add(bytes.len());
        let mut information = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: This only queries the remote address and writes the local descriptor.
        let queried = unsafe {
            VirtualQueryEx(
                process.raw_handle(),
                Some(cursor as *const c_void),
                &mut information,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if queried == 0 {
            break;
        }
        let region_end = (information.BaseAddress as usize).saturating_add(information.RegionSize);
        let chunk_size = region_end
            .saturating_sub(cursor)
            .min(maximum_size - bytes.len());
        if chunk_size == 0 {
            break;
        }
        match read_memory(process, cursor, chunk_size) {
            Ok(chunk) => bytes.extend_from_slice(&chunk),
            Err(error) if bytes.is_empty() => return Err(error),
            Err(_) => break,
        }
    }
    Ok(bytes)
}

fn write_memory(
    process: &RemoteProcess,
    address: usize,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), RemoteHookError> {
    let _ = process;
    const CHUNK_SIZE: usize = 64;
    let mut offset = 0;
    while bytes.len().saturating_sub(offset) >= CHUNK_SIZE {
        let mut chunk = [0_u8; CHUNK_SIZE];
        chunk.copy_from_slice(&bytes[offset..offset + CHUNK_SIZE]);
        crate::writef(address + offset, chunk)
            .map_err(|error| RemoteHookError::DriverMemory { operation, error })?;
        offset += CHUNK_SIZE;
    }
    for byte in &bytes[offset..] {
        crate::writef(address + offset, *byte)
            .map_err(|error| RemoteHookError::DriverMemory { operation, error })?;
        offset += 1;
    }
    Ok(())
}

fn flush_instructions(
    process: &RemoteProcess,
    address: usize,
    size: usize,
) -> Result<(), RemoteHookError> {
    // SAFETY: The range is valid in `process`; flushing does not dereference it locally.
    unsafe { FlushInstructionCache(process.raw_handle(), Some(address as *const c_void), size) }
        .map_err(|source| RemoteHookError::Windows {
            operation: "FlushInstructionCache",
            source,
        })
}

fn ensure_driver_initialized() -> Result<(), RemoteHookError> {
    if crate::process_base() == 0 {
        Err(RemoteHookError::DriverNotInitialized)
    } else {
        Ok(())
    }
}

fn find_and_allocate_near(
    process: &RemoteProcess,
    target: usize,
    size: usize,
) -> Option<*mut c_void> {
    let mut system_info = SYSTEM_INFO::default();
    // SAFETY: `system_info` points to writable local storage.
    unsafe { GetSystemInfo(&mut system_info) };
    let granularity = system_info.dwAllocationGranularity as usize;
    let minimum = system_info.lpMinimumApplicationAddress as usize;
    let maximum = system_info.lpMaximumApplicationAddress as usize;
    let search_start = align_up(
        target.saturating_sub(RELATIVE_RANGE).max(minimum),
        granularity,
    );
    let search_end = target.saturating_add(RELATIVE_RANGE).min(maximum);
    let mut cursor = search_start;
    let mut candidates = Vec::new();

    while cursor < search_end {
        let mut information = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: `information` is writable and `cursor` is only queried, not dereferenced.
        let queried = unsafe {
            VirtualQueryEx(
                process.raw_handle(),
                Some(cursor as *const c_void),
                &mut information,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if queried == 0 {
            break;
        }

        let region_start = information.BaseAddress as usize;
        let region_end = region_start.saturating_add(information.RegionSize);
        if information.State == MEM_FREE {
            let first = align_up(region_start.max(search_start), granularity);
            let last = align_down(region_end.saturating_sub(size).min(search_end), granularity);
            if first <= last && first.saturating_add(size) <= region_end {
                let preferred = align_down(target.clamp(first, last), granularity);
                candidates.push((preferred.abs_diff(target), preferred));
            }
        }

        let next = region_end.max(cursor.saturating_add(granularity));
        if next <= cursor {
            break;
        }
        cursor = next;
    }

    candidates.sort_unstable_by_key(|candidate| candidate.0);
    candidates.into_iter().find_map(|(_, address)| {
        // SAFETY: The address came from a MEM_FREE region and is allocation-granularity aligned.
        let allocation = unsafe {
            VirtualAllocEx(
                process.raw_handle(),
                Some(address as *const c_void),
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        (!allocation.is_null()).then_some(allocation)
    })
}

fn allocate_anywhere(process: &RemoteProcess, size: usize) -> Option<*mut c_void> {
    // SAFETY: A null preferred address requests a fresh allocation.
    let allocation = unsafe {
        VirtualAllocEx(
            process.raw_handle(),
            None,
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    (!allocation.is_null()).then_some(allocation)
}

fn align_up(value: usize, alignment: usize) -> usize {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|value| align_down(value, alignment))
        .unwrap_or(usize::MAX & !(alignment.saturating_sub(1)))
}

fn align_down(value: usize, alignment: usize) -> usize {
    value & !(alignment.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    static HOOK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[unsafe(no_mangle)]
    #[inline(never)]
    extern "system" fn newbase_hook_test_target(value: usize) -> usize {
        std::hint::black_box(value.wrapping_mul(3).wrapping_add(1))
    }

    #[unsafe(no_mangle)]
    #[inline(never)]
    extern "system" fn newbase_hook_test_detour(value: usize) -> usize {
        std::hint::black_box(value.wrapping_add(100))
    }

    fn call_test_target(value: usize) -> usize {
        let function = std::hint::black_box(newbase_hook_test_target as *const () as usize);
        // SAFETY: This address is the test function above or its installed detour.
        let function: extern "system" fn(usize) -> usize = unsafe { std::mem::transmute(function) };
        function(value)
    }

    #[test]
    fn emits_relative_jump_when_destination_is_in_range() {
        let jump = encode_jump(0x1000, 0x2000);
        assert_eq!(jump[0], 0xe9);
        assert_eq!(jump.len(), RELATIVE_JUMP_SIZE);
        let displacement = i32::from_le_bytes(jump[1..].try_into().unwrap());
        assert_eq!(0x1000_i64 + 5 + i64::from(displacement), 0x2000);
    }

    #[test]
    fn emits_register_preserving_absolute_jump_when_destination_is_far() {
        let destination = 0x7fff_1234_5678usize;
        let jump = encode_jump(0x1000, destination);
        assert_eq!(&jump[..6], &[0xff, 0x25, 0, 0, 0, 0]);
        assert_eq!(jump.len(), ABSOLUTE_JUMP_SIZE);
        assert_eq!(
            u64::from_le_bytes(jump[6..].try_into().unwrap()),
            destination as u64
        );
    }

    #[test]
    fn relocates_relative_control_flow_and_rip_relative_memory() {
        // call the MOV; jump back to the CALL; load through RIP-relative memory; loop back.
        let original = [
            0xe8, 2, 0, 0, 0, 0x75, 0xf9, 0x48, 0x8b, 0x05, 0x34, 0x12, 0, 0, 0xe2, 0xf0,
        ];
        let mut decoder = Decoder::with_ip(64, &original, 0x1000, DecoderOptions::NONE);
        let instructions: Vec<_> = decoder.iter().collect();
        let relocated = build_trampoline(&instructions, 0x7fff_0000_0000, 0x1000 + original.len())
            .expect("branches and RIP-relative memory should be relocatable");
        assert!(!relocated.is_empty());
        assert!(relocated.len() > original.len());
    }

    #[test]
    #[ignore = "requires an initialized WinNotify driver"]
    fn installs_disables_and_removes_a_live_detour() {
        let _guard = HOOK_TEST_LOCK.lock().unwrap();
        let process = RemoteProcess::open(std::process::id()).unwrap();
        let target = newbase_hook_test_target as *const () as usize;
        let detour = newbase_hook_test_detour as *const () as usize;
        assert_eq!(call_test_target(7), 22);

        let mut hook = unsafe { RemoteHook::install_detour(&process, target, detour) }.unwrap();
        assert_eq!(hook.kind(), RemoteHookKind::Detour);
        assert_eq!(call_test_target(7), 107);

        let original: extern "system" fn(usize) -> usize =
            unsafe { std::mem::transmute(hook.trampoline_address()) };
        assert_eq!(original(7), 22);

        unsafe { hook.disable() }.unwrap();
        assert_eq!(call_test_target(7), 22);
        unsafe { hook.enable() }.unwrap();
        assert_eq!(call_test_target(7), 107);
        unsafe { hook.remove() }.unwrap();
        assert_eq!(call_test_target(7), 22);
    }

    #[test]
    #[ignore = "requires an initialized WinNotify driver"]
    fn injects_code_in_remote_memory_and_continues_to_original() {
        let _guard = HOOK_TEST_LOCK.lock().unwrap();
        let process = RemoteProcess::open(std::process::id()).unwrap();
        let target = newbase_hook_test_target as *const () as usize;

        // push rax; pop rax -- a harmless position-independent prelude.
        let hook = unsafe { RemoteHook::install_code(&process, target, &[0x50, 0x58]) }.unwrap();
        assert_eq!(hook.kind(), RemoteHookKind::InjectedCode);
        assert_ne!(hook.destination_address(), target);
        assert_eq!(call_test_target(7), 22);
        drop(hook);
        assert_eq!(call_test_target(7), 22);
    }
}
