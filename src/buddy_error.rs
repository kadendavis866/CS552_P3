use core::fmt;

/// Enum representing some possible errors that can occur in the Buddy Memory Allocator.
#[derive(PartialEq)]
pub enum BuddyError {
    NoMemory,
    CorruptedMemoryPool,
}

impl fmt::Debug for BuddyError {
    /// Formats the error message for debugging purposes.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BuddyError::NoMemory => write!(f, "Insufficient memory available"),
            BuddyError::CorruptedMemoryPool => write!(f, "Memory pool is corrupted or invalid")
        }
    }
}

impl fmt::Display for BuddyError {
    /// Formats the error message for display purposes.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}