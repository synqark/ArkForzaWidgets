//! Windows 専用: オーバーレイウィンドウ操作 & 環境情報取得。
//!
//! 一部の関数は現状 main.rs から呼ばれていないが、
//! 将来のウィドウ追加や仕様変更に備えて公開しておく。
#![allow(dead_code)]

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, MonitorFromPoint, HMONITOR, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
    MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_NOREPEAT};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetClientRect, GetForegroundWindow, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowThreadProcessId, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, MSG,
    SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOWNOACTIVATE, WM_HOTKEY, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use crossbeam_channel::Receiver;

use crate::platform::HotkeyEvent;

const CLICKTHROUGH_EX_STYLE: isize =
    (WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize;

/// プロセスを **System DPI Aware** に設定する。
///
/// eframe/winit がイベントループ (= 最初のウィンドウ) を作る前に呼ぶこと。
/// SetProcessDpiAwarenessContext はプロセスで一度しか効かないので、`main()` の先頭で呼び、
/// winit が後から Per-Monitor V2 を設定しようとしても無視させる。
///
/// 目的: 全ウィンドウの `scale_factor` を「システム DPI」に固定する。
/// これにより設定ウィンドウを別倍率モニタへ移動しても `pixels_per_point` が変化せず、
/// egui が異なる ppp 用のフォントアトラス (どちらも `Managed(0)` を共有) を 2 つ持つのを防ぐ。
/// 異なる ppp が共存すると `end_pass` で片方の部分テクスチャ更新が他方のアトラスサイズを
/// 超えて "Partial texture update is outside the bounds" パニックを起こす。
pub fn set_system_dpi_aware() {
    unsafe {
        if let Err(e) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) {
            // 既に設定済み等で失敗しても致命的ではないのでログのみ。
            log::debug!("SetProcessDpiAwarenessContext(SYSTEM_AWARE) failed: {e}");
        }
    }
}

/// `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`
/// を渡されたウィンドウに追加する。
///
/// 戻り値: HWND を取得して `SetWindowLongPtrW` まで呼べたら `true`。
/// 既に同じスタイルが付いていて変更不要だった場合も `true`。
pub fn apply_clickthrough<H: HasWindowHandle>(handle: &H) -> bool {
    match hwnd_of(handle) {
        Some(hwnd) => apply_clickthrough_hwnd(hwnd),
        None => false,
    }
}

/// HWND 直接指定版。子ビューポート (FindWindowW で取得) 向け。
pub fn apply_clickthrough_hwnd(hwnd: HWND) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new = current | CLICKTHROUGH_EX_STYLE;
        if new == current {
            return true;
        }
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
        log::info!(
            "applied clickthrough ex-style: 0x{:X} -> 0x{:X}",
            current as u32,
            new as u32
        );
    }
    true
}

/// `ShowWindow(SW_HIDE/SW_SHOWNOACTIVATE)` で表示/非表示を切り替える。
/// アクティブ化はしない (Forza のフォーカスを奪わない)。
///
/// 戻り値: HWND を取得して `ShowWindow` を呼べたら `true`。
pub fn set_window_visible<H: HasWindowHandle>(handle: &H, visible: bool) -> bool {
    let Some(hwnd) = hwnd_of(handle) else {
        return false;
    };
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
    true
}

/// HWND 直接指定の Show/Hide。
pub fn set_hwnd_visible(hwnd: HWND, visible: bool) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
    true
}

/// タイトル名でトップレベルウィンドウの HWND を検索する。
/// 子ビューポートに HWND 経由でアクセスする手段。
pub fn find_hwnd_by_title(title: &str) -> Option<HWND> {
    let s = HSTRING::from(title);
    unsafe {
        let hwnd = FindWindowW(PCWSTR::null(), PCWSTR(s.as_ptr())).ok()?;
        if hwnd.0.is_null() {
            None
        } else {
            Some(hwnd)
        }
    }
}

/// プライマリディスプレイの物理ピクセルサイズ。
/// 呼び出し前にプロセスが DPI-aware である必要がある (winit が初期化時に行う)。
pub fn primary_display_size() -> Option<(u32, u32)> {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w > 0 && h > 0 {
            Some((w as u32, h as u32))
        } else {
            None
        }
    }
}

/// 現在フォアグラウンドにあるウィンドウのクライアント領域。
/// 戻り値: `(left, top, width, height)`。left/top はスクリーン座標 (物理ピクセル)。
///
/// ゲームがフォアグラウンドのときに呼べば、そのゲームのレンダリング解像度
/// (クライアント領域サイズ) と画面上の原点が得られる。
pub fn foreground_window_rect() -> Option<(i32, i32, u32, u32)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client).ok()?;
        let w = (client.right - client.left) as i32;
        let h = (client.bottom - client.top) as i32;
        if w <= 0 || h <= 0 {
            return None;
        }
        // クライアント左上 (0,0) をスクリーン座標へ変換して原点を得る。
        let mut origin = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(hwnd, &mut origin);
        Some((origin.x, origin.y, w as u32, h as u32))
    }
}

/// 指定したスクリーン座標 (物理ピクセル) が属するモニタの DPI スケール係数を返す。
/// 例: 100% → 1.0、150% → 1.5。取得失敗時は `None`。
///
/// オーバーレイをゲームのモニタへ正しく配置するために使う。設定ウィンドウが
/// 別倍率のモニタにあっても、オーバーレイ側はこの値で独立して換算できる。
pub fn monitor_scale_at(x: i32, y: i32) -> Option<f32> {
    unsafe {
        let monitor: HMONITOR = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return None;
        }
        let mut dpi_x: u32 = 0;
        let mut dpi_y: u32 = 0;
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).ok()?;
        if dpi_x == 0 {
            return None;
        }
        Some(dpi_x as f32 / 96.0)
    }
}

/// 現在フォアグラウンドにあるウィンドウの実行ファイル名 (e.g. "ForzaHorizon6.exe")。
pub fn foreground_process_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        let tid = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if tid == 0 || pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buf = vec![0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        let res = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        if res.is_err() || size == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        let name = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
        Some(name)
    }
}

fn hwnd_of<H: HasWindowHandle>(handle: &H) -> Option<HWND> {
    let wh = handle.window_handle().ok()?;
    match wh.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut core::ffi::c_void)),
        _ => None,
    }
}

/// ホットキー ID (アプリ内で一意ならよい)
const HOTKEY_SAVE: i32 = 1;
const HOTKEY_CLEAR: i32 = 2;

/// グローバルホットキー (Alt+S / Alt+D) を監視する専用スレッドを起動する。
///
/// `RegisterHotKey(None, ...)` はホットキーを呼び出しスレッドに紐づけ、`WM_HOTKEY` を
/// そのスレッドのメッセージキューへ投函する。そのため専用スレッドで `GetMessageW`
/// ループを回す。ゲームがフォアグラウンドでも OS レベルで横取りできる。
///
/// 戻り値の `Receiver` を `try_recv` でポーリングしてイベントを取り出す。
pub fn spawn_hotkey_listener() -> Option<Receiver<HotkeyEvent>> {
    let (tx, rx) = crossbeam_channel::unbounded::<HotkeyEvent>();
    std::thread::Builder::new()
        .name("hotkey-listener".into())
        .spawn(move || unsafe {
            // Alt + S / Alt + D。MOD_NOREPEAT で押しっぱなしの連続発火を防ぐ。
            let mods = MOD_ALT | MOD_NOREPEAT;
            if RegisterHotKey(None, HOTKEY_SAVE, mods, b'S' as u32).is_err() {
                log::warn!("RegisterHotKey Alt+S failed (already in use?)");
            }
            if RegisterHotKey(None, HOTKEY_CLEAR, mods, b'D' as u32).is_err() {
                log::warn!("RegisterHotKey Alt+D failed (already in use?)");
            }
            log::info!("hotkey listener started (Alt+S = save, Alt+D = clear)");

            let mut msg = MSG::default();
            // GetMessageW はエラー時に -1 (BOOL == -1) を返す。0 (WM_QUIT) でループ終了。
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                if msg.message == WM_HOTKEY {
                    let event = match msg.wParam.0 as i32 {
                        HOTKEY_SAVE => Some(HotkeyEvent::SaveProfile),
                        HOTKEY_CLEAR => Some(HotkeyEvent::ClearProfile),
                        _ => None,
                    };
                    if let Some(ev) = event {
                        // 受信側が落ちていたら送信失敗するがスレッドは継続させる。
                        let _ = tx.send(ev);
                    }
                }
            }
        })
        .ok()?;
    Some(rx)
}
