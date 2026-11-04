//! Native Windows back-ends (Win32 via `windows-sys`).

pub mod audio;
pub mod elevate;
pub mod foreground;
pub mod gamepad;
pub mod injector;
pub mod media;
pub mod recorder;
pub mod remapper;

/// Run this process at high priority. A game that keeps every core busy made Windows
/// hold back the G13 threads at normal priority: keys waited 6 ms in the median and
/// over 60 ms one time in ten (up to 0.7 s); at high priority, 0.3 ms and 0.5 ms
/// (measured in game). G Nexus itself uses well under 1 % of the CPU.
pub fn raise_priority() {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, HIGH_PRIORITY_CLASS, SetPriorityClass};
    // SAFETY: the pseudo-handle of the current process needs no closing.
    if unsafe { SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS) } == 0 {
        log::warn!("could not raise the process priority");
    }
}

pub mod temperatures;
pub fn idle_time() -> Option<std::time::Duration> {
    use windows_sys::Win32::{UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO}, System::SystemInformation::GetTickCount};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: the structure is initialized and writable; tick counts share the
    // same wrapping millisecond clock (including after 49.7 days of uptime).
    unsafe { (GetLastInputInfo(&mut info) != 0).then(|| std::time::Duration::from_millis(u64::from(GetTickCount().wrapping_sub(info.dwTime)))) }
}
mod aida64;
