//! Ordinary-key assignment validation, suppression decisions, and action playback.
//! The Windows hook only routes events; this module has no system dependencies.
use std::{collections::{BTreeMap, BTreeSet}, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}, time::Duration};
use indexmap::IndexMap;
use crate::{engine::{action::{Action, MacroMode}, macros::{self, MacroHandle}}, keycode::{KeyCode, MouseButton},
    platform::{InputInjector, PlatformResult}, profiles::format::{ActionDto, MacroDto, macro_from_dto, parse_action}};

pub type Assignments = BTreeMap<KeyCode, Action>;

pub(crate) struct Service {
    #[cfg(windows)]
    inner: Option<crate::platform::windows::remapper::Remapper>,
    #[cfg(windows)]
    injector: Arc<dyn InputInjector>,
}

impl Service {
    pub fn new(_injector: Arc<dyn InputInjector>) -> Self {
        Self { #[cfg(windows)] inner: None, #[cfg(windows)] injector: _injector }
    }
    pub fn configure(&mut self, assignments: Assignments) -> Result<(), String> {
        #[cfg(windows)] {
            if self.inner.is_none() && !assignments.is_empty() {
                self.inner = Some(crate::platform::windows::remapper::Remapper::start(self.injector.clone())?);
            }
            if let Some(inner) = &self.inner { inner.configure(assignments)?; }
            Ok(())
        }
        #[cfg(not(windows))] {
            if assignments.is_empty() { Ok(()) } else { Err("Regular-key assignments currently require Windows".into()) }
        }
    }
    pub fn is_active(&self) -> bool {
        #[cfg(windows)] { self.inner.as_ref().is_some_and(|inner| inner.is_active()) }
        #[cfg(not(windows))] { false }
    }
}

pub fn compile(keys: &BTreeMap<String, ActionDto>, library: &IndexMap<String, MacroDto>) -> Result<Assignments, String> {
    let macros = library.iter().map(|(n, m)| Ok((n.clone(), Arc::new(macro_from_dto(n, m).map_err(|e| e.to_string())?))))
        .collect::<Result<IndexMap<_, _>, String>>()?;
    let mut out = BTreeMap::new();
    for (source, dto) in keys {
        let key = KeyCode::parse(source).ok_or_else(|| format!("Unknown source key: {source}"))?;
        let action = parse_action(dto, &macros)?;
        if matches!(action, Action::Pad(_) | Action::TapDance(_)) {
            return Err("Regular keys support keys, shortcuts, mouse actions and macros".into());
        }
        if out.insert(key, action).is_some() { return Err(format!("Duplicate source key: {source}")); }
    }
    Ok(out)
}

#[derive(Default)]
pub(crate) struct Gate {
    keys: BTreeSet<KeyCode>,
    physical: BTreeSet<KeyCode>,
    suppressed: BTreeSet<KeyCode>,
    blocked: BTreeSet<KeyCode>,
    passthrough: BTreeSet<KeyCode>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Route { Pass, Suppress, Map }

impl Gate {
    pub fn configure(&mut self, keys: impl Iterator<Item = KeyCode>) {
        self.keys = keys.collect();
        self.blocked.extend(self.suppressed.iter().copied());
        self.passthrough.extend(self.physical.difference(&self.suppressed).copied());
    }

    pub fn held(&self, key: KeyCode) -> bool { self.physical.contains(&key) }

    pub fn event(&mut self, key: KeyCode, down: bool) -> Route {
        if down { self.physical.insert(key); } else { self.physical.remove(&key); }
        if self.passthrough.contains(&key) {
            if !down { self.passthrough.remove(&key); }
            return Route::Pass;
        }
        if self.blocked.contains(&key) {
            if !down { self.blocked.remove(&key); self.suppressed.remove(&key); }
            return Route::Suppress;
        }
        if self.suppressed.contains(&key) {
            if !down { self.suppressed.remove(&key); }
            return Route::Map;
        }
        if down && self.keys.contains(&key) { self.suppressed.insert(key); Route::Map }
        else { Route::Pass }
    }
}

/// Share output ownership between simultaneous assignments and macro players.
struct CountedInjector {
    inner: Arc<dyn InputInjector>,
    keys: Mutex<BTreeMap<KeyCode, usize>>,
    buttons: Mutex<BTreeMap<u8, usize>>,
    ownership: Arc<KeyOwnership>,
}

pub(crate) struct KeyOwnership {
    physical: Vec<AtomicBool>,
    outputs: Vec<AtomicUsize>,
}
impl Default for KeyOwnership {
    fn default() -> Self { Self { physical: KeyCode::ALL.iter().map(|_| AtomicBool::new(false)).collect(),
        outputs: KeyCode::ALL.iter().map(|_| AtomicUsize::new(0)).collect() } }
}
impl KeyOwnership {
    pub fn physical(&self, key: KeyCode, down: bool) { self.physical[key as usize].store(down, Ordering::Release); }
    pub fn output_held(&self, key: KeyCode) -> bool { self.outputs[key as usize].load(Ordering::Acquire) > 0 }
}

impl CountedInjector {
    fn count<K: Ord + Copy>(counts: &Mutex<BTreeMap<K, usize>>, key: K, down: bool, send: impl FnOnce() -> PlatformResult) -> PlatformResult {
        let mut counts = counts.lock().unwrap();
        if down {
            let count = counts.entry(key).or_default();
            if *count == 0 { send()?; }
            *count += 1;
        } else if let Some(count) = counts.get_mut(&key) {
            if *count > 1 { *count -= 1; }
            else { send()?; counts.remove(&key); }
        }
        Ok(())
    }
}

impl InputInjector for CountedInjector {
    fn key(&self, key: KeyCode, down: bool) -> PlatformResult {
        let mut counts = self.keys.lock().unwrap();
        let count = counts.get(&key).copied().unwrap_or(0);
        if down {
            if count == 0 && !self.ownership.physical[key as usize].load(Ordering::Acquire) { self.inner.key(key, true)?; }
            counts.insert(key, count + 1);
            self.ownership.outputs[key as usize].store(count + 1, Ordering::Release);
        } else if count > 0 {
            if count == 1 {
                if !self.ownership.physical[key as usize].load(Ordering::Acquire) { self.inner.key(key, false)?; }
                counts.remove(&key);
            } else { counts.insert(key, count - 1); }
            self.ownership.outputs[key as usize].store(count - 1, Ordering::Release);
        }
        Ok(())
    }
    fn mouse_button(&self, button: MouseButton, down: bool) -> PlatformResult {
        Self::count(&self.buttons, button as u8, down, || self.inner.mouse_button(button, down))
    }
    fn mouse_wheel(&self, delta: i32, horizontal: bool) -> PlatformResult { self.inner.mouse_wheel(delta, horizontal) }
    fn mouse_move(&self, dx: i32, dy: i32) -> PlatformResult { self.inner.mouse_move(dx, dy) }
}

pub(crate) struct Player {
    assignments: Assignments,
    held: BTreeSet<KeyCode>,
    macros: BTreeMap<KeyCode, (MacroMode, MacroHandle)>,
    injector: Arc<CountedInjector>,
}

impl Player {
    #[cfg(test)]
    pub fn new(injector: Arc<dyn InputInjector>) -> Self {
        Self::with_ownership(injector, Arc::default())
    }

    pub fn with_ownership(injector: Arc<dyn InputInjector>, ownership: Arc<KeyOwnership>) -> Self {
        Self { assignments: BTreeMap::new(), held: BTreeSet::new(), macros: BTreeMap::new(),
            injector: Arc::new(CountedInjector { inner: injector, keys: Mutex::default(), buttons: Mutex::default(), ownership }) }
    }

    pub fn configure(&mut self, assignments: Assignments) { self.release(); self.assignments = assignments; }

    pub fn event(&mut self, source: KeyCode, down: bool) {
        let Some(action) = self.assignments.get(&source).cloned() else { return };
        let repeat = if down { !self.held.insert(source) } else { !self.held.remove(&source) };
        if !down && repeat { return; }
        let result = match action {
            Action::Key(chord) if down && repeat => {
                // Forward hardware repeat as another down, without acquiring a second hold.
                chord.keys().last().map_or(Ok(()), |k| self.injector.inner.key(*k, true))
            }
            Action::Key(chord) => {
                let keys: Vec<_> = if down { chord.keys().to_vec() } else { chord.keys().iter().rev().copied().collect() };
                keys.into_iter().try_for_each(|key| self.injector.key(key, down))
            }
            Action::MouseButton(button) if !repeat => self.injector.mouse_button(button, down),
            Action::Wheel { delta, horizontal } if down => self.injector.mouse_wheel(delta, horizontal),
            Action::Macro(mac) if !repeat => {
                if down {
                    let running = self.macros.get(&source).is_some_and(|(_, h)| !h.is_finished());
                    if running && mac.mode == MacroMode::Toggle {
                        if let Some((_, h)) = self.macros.remove(&source) { h.cancel(); h.wait(Duration::from_millis(500)); }
                    } else if !running {
                        match macros::spawn(self.injector.clone(), mac.clone(), mac.mode != MacroMode::Once) {
                            Ok(h) => { self.macros.insert(source, (mac.mode, h)); }
                            Err(e) => log::error!("regular-key macro: {e}"),
                        }
                    }
                } else if self.macros.get(&source).is_some_and(|(mode, _)| *mode == MacroMode::RepeatWhileHeld)
                    && let Some((_, h)) = self.macros.remove(&source) { h.cancel(); h.wait(Duration::from_millis(500)); }
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result { log::error!("regular-key assignment: {e}"); }
    }

    pub fn release(&mut self) {
        for (_, handle) in self.macros.values() { handle.cancel(); }
        for (_, handle) in self.macros.values() { handle.wait(Duration::from_millis(500)); }
        self.macros.clear();
        for key in self.held.clone() { self.event(key, false); }
        self.held.clear();
    }
}

impl Drop for Player { fn drop(&mut self) { self.release(); } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::mock::{Injected, RecordingInjector};

    #[test]
    fn gate_passes_default_keys_and_suppresses_both_edges_and_repeats() {
        let mut gate = Gate::default();
        gate.configure([KeyCode::A].into_iter());
        assert_eq!(gate.event(KeyCode::B, true), Route::Pass);
        assert_eq!(gate.event(KeyCode::B, false), Route::Pass);
        assert_eq!(gate.event(KeyCode::A, true), Route::Map);
        assert_eq!(gate.event(KeyCode::A, true), Route::Map);
        assert_eq!(gate.event(KeyCode::A, false), Route::Map);
    }

    #[test]
    fn disabling_held_source_consumes_the_remaining_edges() {
        let mut gate = Gate::default();
        gate.configure([KeyCode::A].into_iter());
        gate.event(KeyCode::A, true);
        gate.configure(std::iter::empty());
        assert_eq!(gate.event(KeyCode::A, true), Route::Suppress);
        assert_eq!(gate.event(KeyCode::A, false), Route::Suppress);
        assert_eq!(gate.event(KeyCode::A, true), Route::Pass);
    }

    #[test]
    fn enabling_a_held_default_key_waits_for_its_original_release() {
        let mut gate = Gate::default();
        gate.event(KeyCode::A, true);
        gate.configure([KeyCode::A].into_iter());
        assert_eq!(gate.event(KeyCode::A, true), Route::Pass);
        assert_eq!(gate.event(KeyCode::A, false), Route::Pass);
        assert_eq!(gate.event(KeyCode::A, true), Route::Map);
    }

    fn assignments(json: &str) -> Assignments {
        compile(&serde_json::from_str(json).unwrap(), &IndexMap::new()).unwrap()
    }

    #[test]
    fn repeats_do_not_leave_an_extra_output_hold() {
        let recorder = Arc::new(RecordingInjector::new());
        let mut player = Player::new(recorder.clone());
        player.configure(assignments(r#"{"A":"B"}"#));
        player.event(KeyCode::A, true);
        player.event(KeyCode::A, true);
        player.event(KeyCode::A, false);
        assert_eq!(recorder.take(), vec![Injected::Key(KeyCode::B, true), Injected::Key(KeyCode::B, true), Injected::Key(KeyCode::B, false)]);
        player.release();
        assert!(recorder.take().is_empty());
    }

    #[test]
    fn simultaneous_shortcuts_share_ctrl_until_the_last_release() {
        let recorder = Arc::new(RecordingInjector::new());
        let mut player = Player::new(recorder.clone());
        player.configure(assignments(r#"{"F6":"Ctrl+C","F7":"Ctrl+V"}"#));
        player.event(KeyCode::F6, true);
        player.event(KeyCode::F7, true);
        player.event(KeyCode::F6, false);
        assert_eq!(recorder.take(), vec![Injected::Key(KeyCode::LeftCtrl, true), Injected::Key(KeyCode::C, true),
            Injected::Key(KeyCode::V, true), Injected::Key(KeyCode::C, false)]);
        player.release();
        assert_eq!(recorder.take(), vec![Injected::Key(KeyCode::V, false), Injected::Key(KeyCode::LeftCtrl, false)]);
    }

    #[test]
    fn reconfiguring_releases_the_old_assignment_and_physical_ctrl_is_preserved() {
        let recorder = Arc::new(RecordingInjector::new());
        let ownership = Arc::new(KeyOwnership::default());
        ownership.physical(KeyCode::LeftCtrl, true);
        let mut player = Player::with_ownership(recorder.clone(), ownership.clone());
        player.configure(assignments(r#"{"F6":"Ctrl+C"}"#));
        player.event(KeyCode::F6, true);
        assert!(ownership.output_held(KeyCode::LeftCtrl));
        player.configure(assignments(r#"{"F6":"F5"}"#));
        assert!(!ownership.output_held(KeyCode::LeftCtrl));
        assert_eq!(recorder.take(), vec![Injected::Key(KeyCode::C, true), Injected::Key(KeyCode::C, false)]);
    }

    #[test]
    fn invalid_and_ambiguous_sources_are_rejected() {
        let keys = serde_json::from_str(r#"{"Unknown":"A"}"#).unwrap();
        assert!(compile(&keys, &IndexMap::new()).is_err());
        let keys = serde_json::from_str(r#"{"Ctrl":"A","LeftCtrl":"B"}"#).unwrap();
        assert!(compile(&keys, &IndexMap::new()).is_err());
        let keys = serde_json::from_str(r#"{"A":{"macro":"missing"}}"#).unwrap();
        assert!(compile(&keys, &IndexMap::new()).is_err());
    }

    #[test]
    fn disabling_cancels_a_repeat_macro_and_releases_its_output() {
        let recorder = Arc::new(RecordingInjector::new());
        let mut player = Player::new(recorder.clone());
        let keys = serde_json::from_str(r#"{"F6":{"macro":"hold"}}"#).unwrap();
        let library = serde_json::from_str(r#"{"hold":{"mode":"repeat_while_held","steps":["down X","wait 60000","up X"]}}"#).unwrap();
        player.configure(compile(&keys, &library).unwrap());
        player.event(KeyCode::F6, true);
        for _ in 0..100 {
            if player.injector.ownership.output_held(KeyCode::X) { break; }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(player.injector.ownership.output_held(KeyCode::X));
        player.configure(BTreeMap::new());
        assert_eq!(recorder.take(), vec![Injected::Key(KeyCode::X, true), Injected::Key(KeyCode::X, false)]);
    }
}
