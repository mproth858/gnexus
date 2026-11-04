//! End-to-end Windows recorder check using F24 (no text or clipboard shortcut).
//! Prints only the test-key result, never other captured input.
#[cfg(windows)]
fn main() {
    use std::{mem::size_of, thread, time::Duration};
    use g13_daemon::{keycode::KeyCode, recording::Recorded};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput};
    let recorder = g13_daemon::platform::start_recording().expect("start recorder");
    if std::env::args().any(|a| a == "--physical") {
        println!("Listening for 60 seconds; press Ctrl+C in the app.");
        for _ in 0..60 {
            thread::sleep(Duration::from_secs(1));
            println!("captured_count={:?}", recorder.captured_count());
        }
        let events = recorder.stop();
        let ctrl = events.iter().any(|(_, e)| matches!(e, Recorded::Key(KeyCode::LeftCtrl | KeyCode::RightCtrl, true)));
        let c = events.iter().any(|(_, e)| *e == Recorded::Key(KeyCode::C, true));
        println!("physical_test: total={}, Ctrl_received={ctrl}, C_received={c}", events.len());
        return;
    }
    let inputs = [0, KEYEVENTF_KEYUP].map(|flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0x87, wScan: 0,
            dwFlags: flags, time: 0, dwExtraInfo: 0x4752_5042 } },
    });
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    thread::sleep(Duration::from_millis(250));
    let count = recorder.captured_count();
    let events = recorder.stop();
    let down = events.iter().any(|(_, e)| *e == Recorded::Key(KeyCode::F24, true));
    let up = events.iter().any(|(_, e)| *e == Recorded::Key(KeyCode::F24, false));
    println!("sent={sent}, captured_count={count:?}, F24_down={down}, F24_up={up}");
    if sent != 2 || !down || !up { std::process::exit(1); }
}

#[cfg(not(windows))]
fn main() { eprintln!("This diagnostic is Windows-only"); std::process::exit(1); }
