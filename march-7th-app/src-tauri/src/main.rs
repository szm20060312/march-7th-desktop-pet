// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--build-info")) && args.next().is_none()
    {
        // This path must exit before any Tauri context, worker or user config.
        println!(
            "{}",
            serde_json::to_string(&march_7th_app_lib::build_info::BUILD_INFO)
                .expect("static build identity")
        );
        return;
    }
    march_7th_app_lib::run()
}
