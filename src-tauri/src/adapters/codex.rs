use super::{AppAdapter, DesktopAppAdapter};
use crate::domain::AppId;

pub type CodexAdapter = DesktopAppAdapter;

pub fn adapter() -> CodexAdapter {
    DesktopAppAdapter::new(AppId::Codex)
}

pub fn as_app_adapter(adapter: &CodexAdapter) -> &dyn AppAdapter {
    adapter
}
