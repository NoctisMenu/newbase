//! Calling functions in an attached 64-bit Windows process.
//!
//! Calls use the Microsoft x64 ABI and run on a newly-created remote thread.
//! The caller is responsible for supplying a valid function address and an
//! argument list matching the target function's signature.

use std::{ffi::c_void, mem::size_of, sync::Arc, time::Duration};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            Diagnostics::Debug::{FlushInstructionCache, ReadProcessMemory, WriteProcessMemory},
            Memory::{
                MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
                PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx, VirtualProtectEx,
            },
            Threading::{
                CreateRemoteThread, GetExitCodeThread, IsWow64Process, LPTHREAD_START_ROUTINE,
                OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
                PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
            },
        },
    },
    core::{BOOL, Error as WindowsError},
};

/// The maximum number of machine-word arguments accepted by [`RemoteProcess::call`].
pub const MAX_REMOTE_ARGUMENTS: usize = 16;

const FUNCTION_OFFSET: i32 = 0;
const ARGUMENTS_OFFSET: i32 = size_of::<u64>() as i32;
const RAX_OFFSET: i32 = (size_of::<u64>() * (1 + MAX_REMOTE_ARGUMENTS)) as i32;
const XMM0_OFFSET: i32 = RAX_OFFSET + size_of::<u64>() as i32;
const COMPLETED_OFFSET: i32 = XMM0_OFFSET + size_of::<u64>() as i32;

/// A raw argument passed according to the Microsoft x64 calling convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RemoteArgument {
    /// An integer, address, handle, or other value passed in a general-purpose register.
    Integer(u64),
    /// A 32-bit floating-point value passed in an XMM register when applicable.
    F32(f32),
    /// A 64-bit floating-point value passed in an XMM register when applicable.
    F64(f64),
}

impl RemoteArgument {
    fn bits(self) -> u64 {
        match self {
            Self::Integer(value) => value,
            Self::F32(value) => u64::from(value.to_bits()),
            Self::F64(value) => value.to_bits(),
        }
    }

    fn is_float(self) -> bool {
        matches!(self, Self::F32(_) | Self::F64(_))
    }

    /// Construct a pointer argument without erasing its type at the call site.
    pub fn pointer<T>(value: *const T) -> Self {
        Self::Integer(value as usize as u64)
    }

    /// Construct a mutable pointer argument without erasing its type at the call site.
    pub fn pointer_mut<T>(value: *mut T) -> Self {
        Self::Integer(value as usize as u64)
    }
}

macro_rules! impl_integer_argument {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for RemoteArgument {
                fn from(value: $ty) -> Self {
                    Self::Integer(value as u64)
                }
            }
        )*
    };
}

impl_integer_argument!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl From<f32> for RemoteArgument {
    fn from(value: f32) -> Self {
        Self::F32(value)
    }
}

impl From<f64> for RemoteArgument {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

/// Values captured from the standard x64 return registers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RemoteCallResult {
    /// The value returned in `RAX` (integer, pointer, or handle returns).
    pub integer: u64,
    /// The raw 64 bits returned in `XMM0` (floating-point returns).
    pub float_bits: u64,
}

impl RemoteCallResult {
    pub fn as_usize(self) -> usize {
        self.integer as usize
    }

    pub fn as_f32(self) -> f32 {
        f32::from_bits(self.float_bits as u32)
    }

    pub fn as_f64(self) -> f64 {
        f64::from_bits(self.float_bits)
    }
}

/// Errors produced while preparing or executing a remote function call.
#[derive(Debug, Error)]
pub enum RemoteCallError {
    #[error("remote function address cannot be null")]
    NullFunction,
    #[error("too many remote arguments: supplied {supplied}, maximum is {maximum}")]
    TooManyArguments { supplied: usize, maximum: usize },
    #[error("process {pid} is 32-bit; remote calls currently require a 64-bit target")]
    UnsupportedTargetArchitecture { pid: u32 },
    #[error("failed to open process {pid}: {source}")]
    OpenProcess { pid: u32, source: WindowsError },
    #[error("Windows operation '{operation}' failed: {source}")]
    Windows {
        operation: &'static str,
        source: WindowsError,
    },
    #[error("'{operation}' transferred {actual} bytes; expected {expected}")]
    PartialTransfer {
        operation: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("remote call did not finish within {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("WaitForSingleObject returned unexpected status {status:#x}")]
    UnexpectedWaitStatus { status: u32 },
    #[error("remote thread exited with code {exit_code} before recording a result")]
    ThreadExited { exit_code: u32 },
}

#[derive(Debug)]
struct ProcessHandle {
    raw: HANDLE,
}

// Windows process handles may be used and closed from any local thread.
unsafe impl Send for ProcessHandle {}
unsafe impl Sync for ProcessHandle {}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: `raw` is an owned handle returned by OpenProcess.
        unsafe {
            let _ = CloseHandle(self.raw);
        }
    }
}

/// An owned handle to a process in which functions can be invoked.
#[derive(Clone, Debug)]
pub struct RemoteProcess {
    pid: u32,
    handle: Arc<ProcessHandle>,
}

impl RemoteProcess {
    /// Open `pid` with the rights required to allocate memory and create a thread.
    pub fn open(pid: u32) -> Result<Self, RemoteCallError> {
        let access = PROCESS_CREATE_THREAD
            | PROCESS_QUERY_INFORMATION
            | PROCESS_VM_OPERATION
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE;

        // SAFETY: No borrowed pointers are passed and the returned handle is owned.
        let raw = unsafe { OpenProcess(access, false, pid) }
            .map_err(|source| RemoteCallError::OpenProcess { pid, source })?;

        let mut wow64 = BOOL::default();
        // SAFETY: `raw` is valid and `wow64` points to writable local memory.
        if let Err(source) = unsafe { IsWow64Process(raw, &mut wow64) } {
            // SAFETY: `raw` has not yet been wrapped in its owner.
            unsafe {
                let _ = CloseHandle(raw);
            }
            return Err(RemoteCallError::Windows {
                operation: "IsWow64Process",
                source,
            });
        }
        if wow64.as_bool() {
            // SAFETY: `raw` has not yet been wrapped in its owner.
            unsafe {
                let _ = CloseHandle(raw);
            }
            return Err(RemoteCallError::UnsupportedTargetArchitecture { pid });
        }

        Ok(Self {
            pid,
            handle: Arc::new(ProcessHandle { raw }),
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn raw_handle(&self) -> HANDLE {
        self.handle.raw
    }

    /// Invoke a function in this process and wait for its return value.
    ///
    /// Integer and pointer return values are captured from `RAX`; floating-point
    /// return values are captured from `XMM0`. Calls may have at most
    /// [`MAX_REMOTE_ARGUMENTS`] arguments. Aggregate/vector arguments and return
    /// values, member functions that do not use the Microsoft x64 ABI, and
    /// variadic functions are not supported.
    ///
    /// If the timeout elapses, a background cleanup thread waits for the remote
    /// thread before releasing its code and context allocations.
    ///
    /// # Safety
    ///
    /// `function` must identify executable code in the target process, and the
    /// supplied arguments must match that function's ABI and remain valid for
    /// the entire call. Calling an invalid address or using the wrong signature
    /// can corrupt or terminate the target process.
    pub unsafe fn call(
        &self,
        function: usize,
        arguments: &[RemoteArgument],
        timeout: Duration,
    ) -> Result<RemoteCallResult, RemoteCallError> {
        if function == 0 {
            return Err(RemoteCallError::NullFunction);
        }
        if arguments.len() > MAX_REMOTE_ARGUMENTS {
            return Err(RemoteCallError::TooManyArguments {
                supplied: arguments.len(),
                maximum: MAX_REMOTE_ARGUMENTS,
            });
        }

        let mut context = CallContext {
            function: function as u64,
            arguments: [0; MAX_REMOTE_ARGUMENTS],
            rax: 0,
            xmm0: 0,
            completed: 0,
            _padding: 0,
        };
        for (destination, argument) in context.arguments.iter_mut().zip(arguments) {
            *destination = argument.bits();
        }

        let context_allocation = RemoteAllocation::allocate(
            self.handle.clone(),
            size_of::<CallContext>(),
            PAGE_READWRITE,
            "VirtualAllocEx(context)",
        )?;
        context_allocation.write(as_bytes(&context), "WriteProcessMemory(context)")?;

        let code = build_call_stub(arguments);
        let code_allocation = RemoteAllocation::allocate(
            self.handle.clone(),
            code.len(),
            PAGE_READWRITE,
            "VirtualAllocEx(code)",
        )?;
        code_allocation.write(&code, "WriteProcessMemory(code)")?;
        code_allocation.make_executable()?;

        // SAFETY: The generated allocation begins with a valid remote-thread
        // entry point matching `unsafe extern "system" fn(*mut c_void) -> u32`.
        let start: LPTHREAD_START_ROUTINE = Some(unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "system" fn(*mut c_void) -> u32>(
                code_allocation.address,
            )
        });
        // SAFETY: All handles and pointers refer to live remote allocations.
        let thread = unsafe {
            CreateRemoteThread(
                self.handle.raw,
                None,
                0,
                start,
                Some(context_allocation.address.cast_const()),
                0,
                None,
            )
        }
        .map(RemoteThread::new)
        .map_err(|source| RemoteCallError::Windows {
            operation: "CreateRemoteThread",
            source,
        })?;

        let wait_ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        // SAFETY: `thread` owns a valid synchronization handle.
        let wait_result = unsafe { WaitForSingleObject(thread.raw, wait_ms) };

        if wait_result == WAIT_TIMEOUT {
            cleanup_after_completion(thread, context_allocation, code_allocation);
            return Err(RemoteCallError::Timeout { timeout });
        }
        if wait_result == WAIT_FAILED {
            let source = WindowsError::from_thread();
            cleanup_after_completion(thread, context_allocation, code_allocation);
            return Err(RemoteCallError::Windows {
                operation: "WaitForSingleObject",
                source,
            });
        }
        if wait_result != WAIT_OBJECT_0 {
            let status = wait_result.0;
            cleanup_after_completion(thread, context_allocation, code_allocation);
            return Err(RemoteCallError::UnexpectedWaitStatus { status });
        }

        context_allocation.read(as_bytes_mut(&mut context), "ReadProcessMemory(context)")?;
        if context.completed != 1 {
            let mut exit_code = 0;
            // SAFETY: `thread` remains a valid thread handle.
            unsafe { GetExitCodeThread(thread.raw, &mut exit_code) }.map_err(|source| {
                RemoteCallError::Windows {
                    operation: "GetExitCodeThread",
                    source,
                }
            })?;
            return Err(RemoteCallError::ThreadExited { exit_code });
        }

        Ok(RemoteCallResult {
            integer: context.rax,
            float_bits: context.xmm0,
        })
    }

    /// Convenience wrapper for calls containing only integer/pointer arguments.
    ///
    /// # Safety
    ///
    /// The same requirements as [`RemoteProcess::call`] apply.
    pub unsafe fn call_integer(
        &self,
        function: usize,
        arguments: &[usize],
        timeout: Duration,
    ) -> Result<usize, RemoteCallError> {
        let arguments: Vec<_> = arguments
            .iter()
            .copied()
            .map(RemoteArgument::from)
            .collect();
        // SAFETY: The caller accepted `call`'s safety contract.
        unsafe { self.call(function, &arguments, timeout) }.map(RemoteCallResult::as_usize)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CallContext {
    function: u64,
    arguments: [u64; MAX_REMOTE_ARGUMENTS],
    rax: u64,
    xmm0: u64,
    completed: u32,
    _padding: u32,
}

struct RemoteAllocation {
    process: Arc<ProcessHandle>,
    address: *mut c_void,
    size: usize,
}

// The allocation is owned and Windows permits these process operations from
// any thread. Its address is never dereferenced in the local process.
unsafe impl Send for RemoteAllocation {}

impl RemoteAllocation {
    fn allocate(
        process: Arc<ProcessHandle>,
        size: usize,
        protection: PAGE_PROTECTION_FLAGS,
        operation: &'static str,
    ) -> Result<Self, RemoteCallError> {
        // SAFETY: `process` is valid; a null preferred address requests a fresh allocation.
        let address = unsafe {
            VirtualAllocEx(
                process.raw,
                None,
                size,
                MEM_COMMIT | MEM_RESERVE,
                protection,
            )
        };
        if address.is_null() {
            return Err(RemoteCallError::Windows {
                operation,
                source: WindowsError::from_thread(),
            });
        }
        Ok(Self {
            process,
            address,
            size,
        })
    }

    fn write(&self, bytes: &[u8], operation: &'static str) -> Result<(), RemoteCallError> {
        let mut written = 0;
        // SAFETY: Source is valid locally and destination covers at least `bytes.len()`.
        unsafe {
            WriteProcessMemory(
                self.process.raw,
                self.address.cast_const(),
                bytes.as_ptr().cast(),
                bytes.len(),
                Some(&mut written),
            )
        }
        .map_err(|source| RemoteCallError::Windows { operation, source })?;
        check_transfer(operation, bytes.len(), written)
    }

    fn read(&self, bytes: &mut [u8], operation: &'static str) -> Result<(), RemoteCallError> {
        let mut read = 0;
        // SAFETY: Destination is valid locally and source covers at least `bytes.len()`.
        unsafe {
            ReadProcessMemory(
                self.process.raw,
                self.address.cast_const(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                Some(&mut read),
            )
        }
        .map_err(|source| RemoteCallError::Windows { operation, source })?;
        check_transfer(operation, bytes.len(), read)
    }

    fn make_executable(&self) -> Result<(), RemoteCallError> {
        let mut previous = PAGE_PROTECTION_FLAGS::default();
        // SAFETY: This allocation is live and `previous` is writable local memory.
        unsafe {
            VirtualProtectEx(
                self.process.raw,
                self.address.cast_const(),
                self.size,
                PAGE_EXECUTE_READ,
                &mut previous,
            )
        }
        .map_err(|source| RemoteCallError::Windows {
            operation: "VirtualProtectEx(code)",
            source,
        })?;
        // SAFETY: The range is live and has just been populated with instructions.
        unsafe {
            FlushInstructionCache(self.process.raw, Some(self.address.cast_const()), self.size)
        }
        .map_err(|source| RemoteCallError::Windows {
            operation: "FlushInstructionCache",
            source,
        })
    }

    fn into_raw(self) -> (Arc<ProcessHandle>, *mut c_void) {
        let this = std::mem::ManuallyDrop::new(self);
        // SAFETY: `this` will not run Drop, so ownership of the Arc can be moved out.
        let process = unsafe { std::ptr::read(&this.process) };
        (process, this.address)
    }
}

impl Drop for RemoteAllocation {
    fn drop(&mut self) {
        // SAFETY: `address` is an owned MEM_RESERVE allocation in `process`.
        unsafe {
            let _ = VirtualFreeEx(self.process.raw, self.address, 0, MEM_RELEASE);
        }
    }
}

struct RemoteThread {
    raw: HANDLE,
}

impl RemoteThread {
    fn new(raw: HANDLE) -> Self {
        Self { raw }
    }

    fn into_raw(self) -> HANDLE {
        let this = std::mem::ManuallyDrop::new(self);
        this.raw
    }
}

// Windows thread handles can be waited on and closed from any local thread.
unsafe impl Send for RemoteThread {}

impl Drop for RemoteThread {
    fn drop(&mut self) {
        // SAFETY: `raw` is an owned handle returned by CreateRemoteThread.
        unsafe {
            let _ = CloseHandle(self.raw);
        }
    }
}

fn cleanup_after_completion(
    thread: RemoteThread,
    context: RemoteAllocation,
    code: RemoteAllocation,
) {
    // Convert to raw resources before spawning. If the OS cannot create the
    // cleanup thread, these resources intentionally leak instead of freeing
    // executable memory while the remote thread may still be using it.
    let thread = thread.into_raw().0 as usize;
    let (context_process, context_address) = context.into_raw();
    let context_address = context_address as usize;
    let (code_process, code_address) = code.into_raw();
    let code_address = code_address as usize;
    let _ = std::thread::Builder::new()
        .name("remote-call-cleanup".to_string())
        .spawn(move || {
            let thread = HANDLE(thread as *mut c_void);
            let context_address = context_address as *mut c_void;
            let code_address = code_address as *mut c_void;
            // SAFETY: The owned handle remains valid for the duration of this wait.
            unsafe {
                let _ = WaitForSingleObject(thread, u32::MAX);
                let _ = VirtualFreeEx(code_process.raw, code_address, 0, MEM_RELEASE);
                let _ = VirtualFreeEx(context_process.raw, context_address, 0, MEM_RELEASE);
                let _ = CloseHandle(thread);
            }
        });
}

fn check_transfer(
    operation: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), RemoteCallError> {
    if actual == expected {
        Ok(())
    } else {
        Err(RemoteCallError::PartialTransfer {
            operation,
            expected,
            actual,
        })
    }
}

fn as_bytes<T>(value: &T) -> &[u8] {
    // SAFETY: The returned slice has the exact bounds and lifetime of `value`.
    unsafe { std::slice::from_raw_parts((value as *const T).cast(), size_of::<T>()) }
}

fn as_bytes_mut<T>(value: &mut T) -> &mut [u8] {
    // SAFETY: The returned slice has the exact bounds and lifetime of `value`.
    unsafe { std::slice::from_raw_parts_mut((value as *mut T).cast(), size_of::<T>()) }
}

/// Build a position-independent Microsoft x64 call stub.
fn build_call_stub(arguments: &[RemoteArgument]) -> Vec<u8> {
    let stack_argument_count = arguments.len().saturating_sub(4);
    let required_stack = 0x20 + stack_argument_count * size_of::<u64>();
    let stack_size = required_stack.next_multiple_of(16) as u32;
    let mut code = Vec::with_capacity(160);

    code.push(0x53); // push rbx
    code.extend_from_slice(&[0x48, 0x89, 0xcb]); // mov rbx, rcx (context)
    code.extend_from_slice(&[0x48, 0x81, 0xec]); // sub rsp, stack_size
    code.extend_from_slice(&stack_size.to_le_bytes());

    // The fifth and later arguments live above the callee's 32-byte shadow space.
    for index in 4..arguments.len() {
        emit_load_rax_from_context(&mut code, argument_offset(index));
        code.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]); // mov [rsp+disp32], rax
        let stack_offset = (0x20 + (index - 4) * size_of::<u64>()) as u32;
        code.extend_from_slice(&stack_offset.to_le_bytes());
    }

    // Load every register slot into its GP register. Floating-point slots are
    // additionally loaded into the corresponding XMM register.
    const GP_LOADS: [[u8; 3]; 4] = [
        [0x48, 0x8b, 0x8b], // mov rcx, [rbx+disp32]
        [0x48, 0x8b, 0x93], // mov rdx, [rbx+disp32]
        [0x4c, 0x8b, 0x83], // mov r8,  [rbx+disp32]
        [0x4c, 0x8b, 0x8b], // mov r9,  [rbx+disp32]
    ];
    for (index, argument) in arguments.iter().take(4).enumerate() {
        code.extend_from_slice(&GP_LOADS[index]);
        code.extend_from_slice(&argument_offset(index).to_le_bytes());
        if argument.is_float() {
            code.extend_from_slice(&[0xf3, 0x0f, 0x7e, 0x83 | ((index as u8) << 3)]);
            code.extend_from_slice(&argument_offset(index).to_le_bytes());
        }
    }

    code.extend_from_slice(&[0x48, 0x8b, 0x83]); // mov rax, [rbx+function]
    code.extend_from_slice(&FUNCTION_OFFSET.to_le_bytes());
    code.extend_from_slice(&[0xff, 0xd0]); // call rax

    code.extend_from_slice(&[0x48, 0x89, 0x83]); // mov [rbx+rax], rax
    code.extend_from_slice(&RAX_OFFSET.to_le_bytes());
    code.extend_from_slice(&[0x66, 0x48, 0x0f, 0x7e, 0x83]); // movq [rbx+xmm0], xmm0
    code.extend_from_slice(&XMM0_OFFSET.to_le_bytes());
    code.extend_from_slice(&[0xc7, 0x83]); // mov dword [rbx+completed], 1
    code.extend_from_slice(&COMPLETED_OFFSET.to_le_bytes());
    code.extend_from_slice(&1u32.to_le_bytes());

    code.extend_from_slice(&[0x48, 0x81, 0xc4]); // add rsp, stack_size
    code.extend_from_slice(&stack_size.to_le_bytes());
    code.extend_from_slice(&[0x31, 0xc0]); // xor eax, eax
    code.push(0x5b); // pop rbx
    code.push(0xc3); // ret
    code
}

fn emit_load_rax_from_context(code: &mut Vec<u8>, offset: i32) {
    code.extend_from_slice(&[0x48, 0x8b, 0x83]); // mov rax, [rbx+disp32]
    code.extend_from_slice(&offset.to_le_bytes());
}

fn argument_offset(index: usize) -> i32 {
    ARGUMENTS_OFFSET + (index * size_of::<u64>()) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "system" fn add_six(
        first: u64,
        second: u64,
        third: u64,
        fourth: u64,
        fifth: u64,
        sixth: u64,
    ) -> u64 {
        first + second + third + fourth + fifth + sixth
    }

    unsafe extern "system" fn mixed_arguments(
        first: u64,
        second: f64,
        third: u64,
        fourth: f32,
        fifth: u64,
    ) -> f64 {
        first as f64 + second + third as f64 + f64::from(fourth) + fifth as f64
    }

    #[test]
    fn context_offsets_match_layout() {
        let context = std::mem::MaybeUninit::<CallContext>::uninit();
        let base = context.as_ptr() as usize;
        // SAFETY: `addr_of!` does not dereference the uninitialized fields.
        unsafe {
            assert_eq!(
                std::ptr::addr_of!((*context.as_ptr()).function) as usize - base,
                FUNCTION_OFFSET as usize
            );
            assert_eq!(
                std::ptr::addr_of!((*context.as_ptr()).arguments) as usize - base,
                ARGUMENTS_OFFSET as usize
            );
            assert_eq!(
                std::ptr::addr_of!((*context.as_ptr()).rax) as usize - base,
                RAX_OFFSET as usize
            );
            assert_eq!(
                std::ptr::addr_of!((*context.as_ptr()).xmm0) as usize - base,
                XMM0_OFFSET as usize
            );
            assert_eq!(
                std::ptr::addr_of!((*context.as_ptr()).completed) as usize - base,
                COMPLETED_OFFSET as usize
            );
        }
    }

    #[test]
    fn stack_allocation_is_aligned_for_every_supported_argument_count() {
        for count in 0..=MAX_REMOTE_ARGUMENTS {
            let arguments = vec![RemoteArgument::Integer(0); count];
            let stub = build_call_stub(&arguments);
            let stack_size = u32::from_le_bytes(stub[7..11].try_into().unwrap());
            assert_eq!(stack_size % 16, 0);
            assert!(stack_size as usize >= 0x20 + count.saturating_sub(4) * 8);
        }
    }

    #[test]
    fn conversions_preserve_argument_bits() {
        assert_eq!(RemoteArgument::from(-1_i32).bits(), u64::MAX);
        assert_eq!(RemoteArgument::from(42_usize).bits(), 42);
        assert_eq!(
            RemoteArgument::from(1.25_f32).bits(),
            u64::from(1.25_f32.to_bits())
        );
        assert_eq!(RemoteArgument::from(2.5_f64).bits(), 2.5_f64.to_bits());
    }

    #[test]
    fn calls_integer_function_in_current_process() {
        let process = RemoteProcess::open(std::process::id()).unwrap();
        let result = unsafe {
            process.call_integer(
                add_six as *const () as usize,
                &[1, 2, 3, 4, 5, 6],
                Duration::from_secs(2),
            )
        }
        .unwrap();
        assert_eq!(result, 21);
    }

    #[test]
    fn calls_mixed_float_function_in_current_process() {
        let process = RemoteProcess::open(std::process::id()).unwrap();
        let result = unsafe {
            process.call(
                mixed_arguments as *const () as usize,
                &[
                    1_u64.into(),
                    2.5_f64.into(),
                    3_u64.into(),
                    4.25_f32.into(),
                    5_u64.into(),
                ],
                Duration::from_secs(2),
            )
        }
        .unwrap();
        assert_eq!(result.as_f64(), 15.75);
    }
}
