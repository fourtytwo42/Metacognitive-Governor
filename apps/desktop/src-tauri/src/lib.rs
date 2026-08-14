#[tauri::command]
const fn runtime_label() -> &'static str {
    "MCG desktop scaffold"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the native desktop application.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![runtime_label])
        .run(tauri::generate_context!())
        .expect("error while running MCG desktop application");
}
