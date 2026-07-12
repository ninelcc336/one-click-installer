use super::{AppAdapter, DesktopAppAdapter};
use crate::domain::AppId;

pub type CcSwitchAdapter = DesktopAppAdapter;

pub fn adapter() -> CcSwitchAdapter {
    DesktopAppAdapter::new(AppId::CcSwitch)
}

pub fn as_app_adapter(adapter: &CcSwitchAdapter) -> &dyn AppAdapter {
    adapter
}
