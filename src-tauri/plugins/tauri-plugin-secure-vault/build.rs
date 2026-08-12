const COMMANDS: &[&str] = &["store", "load", "delete"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
