//! Read-only G915 discovery check: never calls the settings writer.
fn main() {
    match g13_daemon::keyboard::probe() {
        Ok(status) => println!("{}", serde_json::to_string_pretty(&status).unwrap()),
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
}
