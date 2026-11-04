//! Windows-wide ordinary-key remapping, enabled only during a G915 session.
//! No HID handle is opened here. Hook callbacks only decide suppression and
//! enqueue work; macro playback and SendInput run on a separate thread.
use std::{cell::RefCell, collections::BTreeSet, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Receiver, Sender}},
    thread::{self, JoinHandle}, time::Duration};
use windows_sys::Win32::{Foundation::{LPARAM, LRESULT, WPARAM}, System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE,
        PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        WM_APP, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP}};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, MapVirtualKeyW, MAPVK_VSC_TO_VK_EX};
use crate::{keycode::{KeyCode, WinKey}, platform::InputInjector, remap::{Assignments, Gate, KeyOwnership, Player, Route}};

const WAKE: u32 = WM_APP + 37;
enum Run { Configure(Assignments, Sender<Result<(), String>>), Key(KeyCode, bool), Release, Stop }
enum Command { Configure(Assignments, Sender<Result<(), String>>), Stop }

struct Context {
    gate: Gate,
    configured: BTreeSet<KeyCode>,
    output: Sender<Run>,
    active: Arc<AtomicBool>,
    recording: bool,
    ownership: Arc<KeyOwnership>,
}

thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let event = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if event.dwExtraInfo != super::injector::INJECT_TAG
            && let Some(key) = super::recorder::key_of(event)
            && let Some(down) = match wparam as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => Some(true), WM_KEYUP | WM_SYSKEYUP => Some(false), _ => None,
            }
        {
            let suppress = CONTEXT.with(|context| {
                let mut context = context.borrow_mut();
                let Some(context) = context.as_mut() else { return false };
                let recording = super::recorder::is_recording();
                if recording != context.recording {
                    context.recording = recording;
                    context.gate.configure(if recording { BTreeSet::new() } else { context.configured.clone() }.into_iter());
                    if recording { let _ = context.output.send(Run::Release); }
                }
                // Always available, even if Pause itself has an assignment.
                if down && key == KeyCode::Pause
                    && (context.gate.held(KeyCode::LeftCtrl) || context.gate.held(KeyCode::RightCtrl))
                    && (context.gate.held(KeyCode::LeftAlt) || context.gate.held(KeyCode::RightAlt)) {
                    context.configured.clear();
                    context.gate.configure(std::iter::empty());
                    context.active.store(false, Ordering::Release);
                    let _ = context.output.send(Run::Release);
                    return false;
                }
                match context.gate.event(key, down) {
                    Route::Pass => {
                        context.ownership.physical(key, down);
                        // A held assignment still owns this modifier/output. Its
                        // eventual release will send the key-up when the user is no longer holding it.
                        !down && context.ownership.output_held(key)
                    }
                    Route::Suppress => true,
                    Route::Map => {
                        if context.output.send(Run::Key(key, down)).is_err() {
                            context.active.store(false, Ordering::Release);
                            context.configured.clear();
                            context.gate.configure(std::iter::empty());
                        }
                        true
                    }
                }
            });
            if suppress { return 1; }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

pub struct Remapper {
    commands: Sender<Command>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
    active: Arc<AtomicBool>,
}

impl Remapper {
    pub fn start(injector: Arc<dyn InputInjector>) -> Result<Self, String> {
        let active = Arc::new(AtomicBool::new(false));
        let thread_active = active.clone();
        let (commands, rx) = mpsc::channel();
        let (ready, start) = mpsc::channel();
        let thread = thread::Builder::new().name("regular-key-hook".into()).spawn(move || {
            let (output, events) = mpsc::channel();
            let ownership = Arc::new(KeyOwnership::default());
            let player_ownership = ownership.clone();
            let player = match thread::Builder::new().name("regular-key-actions".into()).spawn(move || run(events, injector, player_ownership)) {
                Ok(thread) => thread, Err(e) => { let _ = ready.send(Err(e.to_string())); return; }
            };
            CONTEXT.with(|c| *c.borrow_mut() = Some(Context { gate: Gate::default(), configured: BTreeSet::new(),
                output: output.clone(), active: thread_active.clone(), recording: false, ownership: ownership.clone() }));
            unsafe {
                // Seed keys already held before the first assignment is enabled.
                for key in KeyCode::ALL {
                    let vk = match key.win() { WinKey::Vk(vk) => u32::from(vk),
                        WinKey::Scan { code, extended } => MapVirtualKeyW(u32::from(code) | if extended { 0xE000 } else { 0 }, MAPVK_VSC_TO_VK_EX) };
                    if vk != 0 && GetAsyncKeyState(vk as i32) < 0 {
                        CONTEXT.with(|c| { if let Some(c) = c.borrow_mut().as_mut() { c.gate.event(*key, true); } });
                        ownership.physical(*key, true);
                    }
                }
                let mut message: MSG = std::mem::zeroed();
                PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                let handle = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), GetModuleHandleW(std::ptr::null()), 0);
                if handle.is_null() {
                    let _ = ready.send(Err(format!("Could not start regular-key remapping: {}", std::io::Error::last_os_error())));
                } else {
                    let _ = ready.send(Ok(GetCurrentThreadId()));
                    'messages: while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                        while let Ok(command) = rx.try_recv() {
                            match command {
                                Command::Stop => break 'messages,
                                Command::Configure(assignments, ack) => {
                                    CONTEXT.with(|c| {
                                        if let Some(c) = c.borrow_mut().as_mut() {
                                            c.configured = assignments.keys().copied().collect();
                                            c.gate.configure(c.configured.clone().into_iter());
                                            c.recording = false;
                                        }
                                    });
                                    thread_active.store(!assignments.is_empty(), Ordering::Release);
                                    let _ = output.send(Run::Configure(assignments, ack));
                                }
                            }
                        }
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    UnhookWindowsHookEx(handle);
                }
            }
            thread_active.store(false, Ordering::Release);
            CONTEXT.with(|c| *c.borrow_mut() = None);
            let _ = output.send(Run::Stop);
            let _ = player.join();
        }).map_err(|e| e.to_string())?;
        match start.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread_id)) => Ok(Self { commands, thread_id, thread: Some(thread), active }),
            Ok(Err(error)) => { let _ = thread.join(); Err(error) }
            Err(error) => Err(format!("Regular-key remapper startup: {error}")),
        }
    }

    pub fn configure(&self, assignments: Assignments) -> Result<(), String> {
        let (ack, ready) = mpsc::channel();
        self.commands.send(Command::Configure(assignments, ack)).map_err(|e| e.to_string())?;
        if unsafe { PostThreadMessageW(self.thread_id, WAKE, 0, 0) } == 0 {
            return Err("Could not update regular-key remapping".into());
        }
        ready.recv_timeout(Duration::from_secs(3)).map_err(|e| format!("Regular-key update: {e}"))?
    }

    pub fn is_active(&self) -> bool { self.active.load(Ordering::Acquire) }
}

fn run(events: Receiver<Run>, injector: Arc<dyn InputInjector>, ownership: Arc<KeyOwnership>) {
    let mut player = Player::with_ownership(injector, ownership);
    while let Ok(event) = events.recv() {
        match event {
            Run::Configure(assignments, ack) => { player.configure(assignments); let _ = ack.send(Ok(())); }
            Run::Key(key, down) => player.event(key, down),
            Run::Release => player.release(),
            Run::Stop => break,
        }
    }
    player.release();
}

impl Drop for Remapper {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        unsafe { PostThreadMessageW(self.thread_id, WAKE, 0, 0); }
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}
