// Compiled directly with rustc by the Node regression: production std-only core.
#[path = "../../src-tauri/src/data_directory.rs"]
mod data_directory;

use data_directory::{acquire, DataFile};
use std::io::{self, Write};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let root = args.next().expect("owned temporary root").into();
    let mode = args.next().expect("mode");
    let directory = match acquire(Some(root)) {
        Ok(directory) => directory,
        Err(data_directory::AlreadyRunning) => {
            println!("alreadyRunning");
            return;
        }
    };
    if let Some(code) = directory.diagnostic() {
        assert!(directory.path(DataFile::Desktop).is_none());
        assert!(directory.path(DataFile::Characters).is_none());
        assert!(directory.path(DataFile::Reminders).is_none());
        println!("unavailable:{code}");
        return;
    }
    // A visible stand-in for service initialization, strictly after qualification.
    // Existing config bytes are never touched by this fixture.
    std::fs::write(
        directory.root().unwrap().join("fixture-started"),
        b"started",
    )
    .unwrap();
    println!("ready");
    io::stdout().flush().unwrap();
    if mode == "hold" {
        let mut instruction = String::new();
        io::stdin().read_line(&mut instruction).unwrap();
        assert_eq!(instruction.trim(), "release");
    } else {
        assert_eq!(mode, "probe");
    }
    drop(directory);
}
