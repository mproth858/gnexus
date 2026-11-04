//! Read-only sensor inventory, using exactly the provider used by the G13 LCD.
use g13_daemon::platform::temperatures::{self, Kind};
fn main() {
    let readings = temperatures::provider().read();
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "cpu": temperatures::primary(&readings, Kind::Cpu),
        "gpu": temperatures::primary(&readings, Kind::Gpu),
        "readings": readings,
    })).unwrap());
}
