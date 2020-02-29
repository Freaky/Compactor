// Helper functions for handling the Windows console from a GUI context.
//
// Windows subsystem applications must explicitly attach to an existing console
// before stdio works, and if not available, create their own if they wish to
// print anything.
//
// These functions enable that, primarily for the purposes of displaying Rust
// panics.

use winapi::um::consoleapi::AllocConsole;
use winapi::um::wincon::{AttachConsole, FreeConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS};

/// Check if we're attached to an existing Windows console
pub fn is_attached() -> bool {
    unsafe { !GetConsoleWindow().is_null() }
}

/// Try to attach to an existing Windows console, if necessary.
///
/// It's normally a no-brainer to call this - it just makes println! and friends
/// work as expected, without cluttering the screen with a console in the general
/// case.
pub fn attach() -> bool {
    if is_attached() {
        return true;
    }

    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

/// Try to attach to a console, and if not, allocate ourselves a new one.
pub fn alloc() -> bool {
    if attach() {
        return true;
    }

    unsafe { AllocConsole() != 0 }
}

/// Free any allocated console, if any.
pub fn free() {
    unsafe { FreeConsole() };
}
