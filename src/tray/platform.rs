//! OS-specific bits of tray mode.

/// Windows: a console-subsystem binary launched from Explorer drags an empty console window
/// along. When we are the only process attached to it, let it go.
pub fn detach_console_if_gui_launch() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Console::{FreeConsole, GetConsoleProcessList};
        let mut pids = [0u32; 4];
        let n = GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32);
        if n <= 1 {
            FreeConsole();
        }
    }
}

/// macOS: creating the status item from inside `NewEvents(Init)` needs a nudge to render.
pub fn wake_main_run_loop() {
    #[cfg(target_os = "macos")]
    {
        use objc2_core_foundation::CFRunLoop;
        if let Some(rl) = CFRunLoop::main() {
            CFRunLoop::wake_up(&rl);
        }
    }
}
