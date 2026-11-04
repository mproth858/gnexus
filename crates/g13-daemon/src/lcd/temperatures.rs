//! Sensor polling never runs on the input/HID engine thread. Cached readings
//! expire, so a failed or stopped provider cannot leave an old temperature up.
use std::{sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc::{self, RecvTimeoutError, Sender}}, thread, time::{Duration, Instant}};
use crate::platform::temperatures::{self, Reading};

const EXPIRES: Duration = Duration::from_secs(6);
type Sample = Arc<Mutex<Option<(Vec<Reading>, Instant)>>>;
pub struct Monitor { sample: Sample, wanted: Arc<AtomicBool>, _stop: Sender<()> }
impl Monitor {
    pub fn new() -> Self {
        let sample: Sample = Arc::new(Mutex::new(None));
        let wanted = Arc::new(AtomicBool::new(false));
        let (stop, commands) = mpsc::channel();
        let shared = sample.clone();
        let active = wanted.clone();
        if let Err(error) = thread::Builder::new().name("g13-temps".into()).spawn(move || {
            let mut provider = temperatures::provider();
            loop {
                match commands.recv_timeout(Duration::from_secs(2)) {
                    Err(RecvTimeoutError::Timeout) => {}
                    _ => return,
                }
                if active.load(Ordering::Relaxed) {
                    let readings = provider.read();
                    *shared.lock().unwrap() = Some((readings, Instant::now()));
                }
            }
        }) { log::warn!("Temperature monitoring unavailable: {error}"); }
        Self { sample, wanted, _stop: stop }
    }

    pub fn set_active(&self, active: bool) { self.wanted.store(active, Ordering::Relaxed); }
    pub fn readings(&self) -> Vec<Reading> {
        fresh(self.sample.lock().unwrap().as_ref(), Instant::now())
    }
}
fn fresh(sample: Option<&(Vec<Reading>, Instant)>, now: Instant) -> Vec<Reading> {
    sample.filter(|(_, at)| now.saturating_duration_since(*at) <= EXPIRES).map(|(readings, _)| readings.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_samples_disappear_instead_of_claiming_to_be_live() {
        let at = Instant::now();
        let sample = (vec![Reading { id: "cpu".into(), name: "CPU".into(), hardware: "CPU".into(), kind: temperatures::Kind::Cpu, celsius: 60.0 }], at);
        assert_eq!(fresh(Some(&sample), at).len(), 1);
        assert!(fresh(Some(&sample), at + EXPIRES + Duration::from_secs(1)).is_empty());
        assert!(fresh(None, at).is_empty());
    }
}
