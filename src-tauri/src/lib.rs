pub mod adapters;
pub mod commands;
pub mod coordinator;
pub mod domain;
pub mod download;
pub mod manifest;
pub mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manifest = manifest::Manifest::from_json(include_str!("../../assets/manifest.json"))
        .expect("内置版本清单必须是有效 JSON");
    tauri::Builder::default()
        .manage(commands::InstallerState { manifest })
        .invoke_handler(tauri::generate_handler![
            commands::get_preflight,
            commands::start_install,
            commands::open_log_folder,
            commands::launch_app,
            commands::choose_install_dir
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}

pub fn app_name() -> &'static str {
    "Codex 与 CC Switch 一键安装器"
}

#[cfg(test)]
mod tests {
    use super::app_name;

    #[test]
    fn app_name_is_stable() {
        assert_eq!(app_name(), "Codex 与 CC Switch 一键安装器");
    }
}
