//! Read-only Yeti GX interface, controls, lighting capabilities and Windows meter.
fn main() {
    match g13_daemon::microphone::probe() {
        Ok(status) => println!("{}", serde_json::to_string_pretty(&status).unwrap()),
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
}
