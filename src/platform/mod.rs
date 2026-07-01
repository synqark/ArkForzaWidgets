//! プラットフォーム依存の小ユーティリティ。
//!
//! Windows 以外 (開発用 Linux ビルド等) では no-op の実装を提供する。

/// グローバルホットキーで発生するイベント。
/// ゲームがフォアグラウンドのときでも受け取れるよう、OS のホットキー機構を使う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// Alt+S: 現在の車のダイノ/ギア比をプロファイルとして保存
    SaveProfile,
    /// Alt+D: 現在の車の保存済みプロファイルをクリア
    ClearProfile,
}

#[cfg(windows)]
#[path = "windows.rs"]
mod win;

#[cfg(windows)]
#[allow(unused_imports)]
pub use self::win::{
    apply_clickthrough, apply_clickthrough_hwnd, find_hwnd_by_title, foreground_process_name,
    foreground_window_rect, monitor_scale_at, primary_display_size, set_hwnd_visible,
    set_system_dpi_aware, set_window_visible, spawn_hotkey_listener,
};
#[cfg(windows)]
pub use ::windows::Win32::Foundation::HWND;

#[cfg(not(windows))]
#[allow(dead_code)]
mod stub {
    use super::HotkeyEvent;
    use crossbeam_channel::Receiver;
    use raw_window_handle::HasWindowHandle;
    /// 非 Windows 環境用の HWND ダミー型 (Windows 版の HWND と同様 .0 が比較できるよう usize)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct HWND(pub usize);
    pub fn apply_clickthrough<H: HasWindowHandle>(_: &H) -> bool {
        false
    }
    pub fn apply_clickthrough_hwnd(_: HWND) -> bool {
        false
    }
    pub fn set_window_visible<H: HasWindowHandle>(_: &H, _visible: bool) -> bool {
        false
    }
    pub fn set_hwnd_visible(_: HWND, _visible: bool) -> bool {
        false
    }
    pub fn find_hwnd_by_title(_: &str) -> Option<HWND> {
        None
    }
    pub fn primary_display_size() -> Option<(u32, u32)> {
        None
    }
    pub fn foreground_window_rect() -> Option<(i32, i32, u32, u32)> {
        None
    }
    pub fn monitor_scale_at(_x: i32, _y: i32) -> Option<f32> {
        None
    }
    pub fn set_system_dpi_aware() {}
    pub fn foreground_process_name() -> Option<String> {
        None
    }
    pub fn spawn_hotkey_listener() -> Option<Receiver<HotkeyEvent>> {
        None
    }
}
#[cfg(not(windows))]
pub use stub::*;
