pub mod app_state;
pub mod commands;
pub mod domain;
pub mod scanner;
pub mod skilldoc;
pub mod stats;
pub mod store;
pub mod translate;
pub mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let project_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let state = app_state::default_shared_app_state(project_dir);

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::list_abilities,
            commands::refresh_scan,
            commands::refresh_stats,
            commands::get_ability,
            commands::update_user_data,
            commands::copy_call_template,
            commands::open_skill_detail_window,
            commands::list_skill_files,
            commands::read_skill_file,
            commands::get_translation_state,
            commands::translate_skill_file
        ])
        .run(tauri::generate_context!())
        .expect("运行 Codex Atlas Tauri 应用失败");
}
