// Compiled directly with rustc by the Node regression: production std-only lock core.
#[path = "../../src-tauri/src/data_lock.rs"]
mod data_lock;

use data_lock::acquire;
use std::io::{self, Write};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let root = args.next().expect("owned temporary root").into();
    let mode = args.next().expect("mode");
    let directory = match acquire(Some(root)) {
        Ok(directory) => directory,
        Err(data_lock::AlreadyRunning) => {
            println!("alreadyRunning");
            return;
        }
    };
    if let data_lock::LockedRoot::Unavailable(code) = &directory {
        println!("unavailable:{code}");
        return;
    }
    // A visible stand-in for service initialization, strictly after qualification.
    // Existing config bytes are never touched by this fixture.
    let data_lock::LockedRoot::Ready { root, .. } = &directory else {
        unreachable!()
    };
    std::fs::write(root.join("fixture-started"), b"started").unwrap();
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
