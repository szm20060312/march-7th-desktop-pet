mod build_identity_support;

fn main() {
    build_identity_support::embed();
    tauri_build::build()
}
