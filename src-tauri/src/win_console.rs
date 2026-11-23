//! The only module of the project with `unsafe` code (Windows only): three
//! Win32 calls without a safe wrapper in our dependencies – attach to / open a
//! console, and read the cursor clip rectangle. Every call has a SAFETY note.
//! The workspace denies `unsafe_code`; a test in console.rs checks that no
//! other file allows it.
#![cfg(windows)]
#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole, SetConsoleTitleW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClipCursor, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

/// `AttachConsole(ATTACH_PARENT_PROCESS)`: the console of the terminal the app
/// was started from, if any.
pub fn attach_parent() -> bool {
    // SAFETY: no pointers; fails harmlessly (returns 0) when there is no
    // parent console or the process already has one.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

/// `AllocConsole` plus a window title: a console window of its own.
pub fn alloc_with_title(title: &str) -> bool {
    // SAFETY: no pointers; fails (returns 0) if the process already has a console.
    let ok = unsafe { AllocConsole() != 0 };
    if ok {
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that lives until the
        // call returns; the function only reads it.
        unsafe { SetConsoleTitleW(wide.as_ptr()) };
    }
    ok
}

/// Whether nobody confines the cursor: `GetClipCursor` against the virtual
/// screen. A game in mouse look confines it to its window or a point.
/// `None` if the rectangle cannot be read.
pub fn pointer_free() -> Option<bool> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `r` is a valid, writable RECT for the duration of the call.
    if unsafe { GetClipCursor(&mut r) } == 0 {
        return None;
    }
    // SAFETY: plain value queries, no pointers.
    let screen = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    Some(crate::console::clip_is_free(
        (r.left, r.top, r.right, r.bottom),
        screen,
    ))
}
