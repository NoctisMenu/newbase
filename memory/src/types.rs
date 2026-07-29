use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    NotInitialized,
    ReadFailed,
    WriteFailed,
    InvalidAddress,
    InvalidSize,
    ProcessNotFound,
    SharedMemoryNotFound,
    PatternNotFound,
    AllocationFailed,
    ProtectionFailed,
    InjectionFailed,
    Timeout,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "memory driver not initialized"),
            Self::ReadFailed => write!(f, "failed to read memory"),
            Self::WriteFailed => write!(f, "failed to write memory"),
            Self::InvalidAddress => write!(f, "invalid address"),
            Self::InvalidSize => write!(f, "invalid size"),
            Self::ProcessNotFound => write!(f, "process not found"),
            Self::SharedMemoryNotFound => write!(f, "shared memory not found"),
            Self::PatternNotFound => write!(f, "pattern not found"),
            Self::AllocationFailed => write!(f, "allocation failed"),
            Self::ProtectionFailed => write!(f, "protection change failed"),
            Self::InjectionFailed => write!(f, "worker injection failed"),
            Self::Timeout => write!(f, "operation timed out"),
        }
    }
}

impl std::error::Error for MemoryError {}
