//! Actual Win32 suppression/output check using non-text F24 -> F23.
#[cfg(windows)]
mod windows {
    use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc}, thread, time::Duration};
    use g13_daemon::{platform::windows::{injector::SendInputInjector, remapper::Remapper}, remap};
    use windows_sys::Win32::{Foundation::{LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput},
        WindowsAndMessaging::{CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, PeekMessageW, PM_NOREMOVE,
            PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_QUIT}}};
    static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static REPLACEMENT: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "system" fn observe(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if code >= 0 {
            let event = unsafe { &*(lp as *const KBDLLHOOKSTRUCT) };
            if event.vkCode == 0x87 { ORIGINAL.fetch_add(1, Ordering::SeqCst); }
            if event.vkCode == 0x86 { REPLACEMENT.fetch_add(1, Ordering::SeqCst); }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
    }

    fn send(flags: &[u32]) {
        let inputs: Vec<_> = flags.iter().map(|flags| INPUT { r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0x87, wScan: 0, dwFlags: *flags, time: 0, dwExtraInfo: 0x4752_5042 } } }).collect();
        assert_eq!(unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) }, inputs.len() as u32);
        thread::sleep(Duration::from_millis(150));
    }

    pub fn main() {
        // Install the observer first, downstream of the remapper, so suppressed
        // originals never reach it. Our tagged replacement events must pass through.
        let (ready, started) = mpsc::channel();
        let observer = thread::spawn(move || unsafe {
            let mut msg: MSG = std::mem::zeroed();
            PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(observe), GetModuleHandleW(std::ptr::null()), 0);
            assert!(!hook.is_null());
            ready.send(GetCurrentThreadId()).unwrap();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
        });
        let id = started.recv().unwrap();
        let remapper = Remapper::start(Arc::new(SendInputInjector::new())).unwrap();
        let mappings = || remap::compile(&serde_json::from_str(r#"{"F24":"F23","F23":"F22"}"#).unwrap(), &Default::default()).unwrap();
        remapper.configure(mappings()).unwrap();
        send(&[0, KEYEVENTF_KEYUP]);
        assert_eq!(ORIGINAL.load(Ordering::SeqCst), 0, "source was not suppressed");
        assert_eq!(REPLACEMENT.load(Ordering::SeqCst), 2, "replacement down/up missing");
        remapper.configure(Default::default()).unwrap();
        send(&[0, KEYEVENTF_KEYUP]);
        assert_eq!(ORIGINAL.load(Ordering::SeqCst), 2, "default key was not restored");
        remapper.configure(mappings()).unwrap();
        send(&[0]);
        remapper.configure(Default::default()).unwrap();
        send(&[KEYEVENTF_KEYUP]);
        assert_eq!(ORIGINAL.load(Ordering::SeqCst), 2, "consumed source release leaked");
        assert_eq!(REPLACEMENT.load(Ordering::SeqCst), 4, "held replacement was not released on disable");
        remapper.configure(mappings()).unwrap();
        let recorder = g13_daemon::platform::start_recording().unwrap();
        send(&[0, KEYEVENTF_KEYUP]);
        let capture = recorder.stop();
        assert_eq!(ORIGINAL.load(Ordering::SeqCst), 4, "recording did not pause overrides");
        assert!(capture.iter().any(|(_, event)| *event == g13_daemon::recording::Recorded::Key(g13_daemon::keycode::KeyCode::F24, true)));
        send(&[0, KEYEVENTF_KEYUP]);
        assert_eq!(REPLACEMENT.load(Ordering::SeqCst), 6, "overrides did not resume after recording");
        assert_eq!(ORIGINAL.load(Ordering::SeqCst), 4);
        drop(remapper);
        unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0); }
        observer.join().unwrap();
        println!("Windows remapping: suppression, output, no recursive chaining, defaults, held releases, recording pause/resume passed.");
    }
}
#[cfg(windows)]
fn main() { windows::main(); }
#[cfg(not(windows))]
fn main() { eprintln!("Windows-only diagnostic"); std::process::exit(1); }
