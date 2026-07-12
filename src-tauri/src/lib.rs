pub mod domain;
pub mod manifest;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
