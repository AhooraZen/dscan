pub mod commands;
pub mod state;

use commands::*;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            start_scan,
            poll_progress,
            get_treemap_data,
            get_extension_legend,
            cancel_scan,
            pause_scan,
            open_in_file_manager,
            get_system_drives,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
