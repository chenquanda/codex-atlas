pub mod commands;
pub mod domain;
pub mod scanner;
pub mod stats;
pub mod store;
pub mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::health])
        .run(tauri::generate_context!())
        .expect("运行 Codex Atlas Tauri 应用失败");
}
