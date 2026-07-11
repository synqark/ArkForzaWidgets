#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod platform;
mod state;
mod telemetry;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use crossbeam_channel::{bounded, Receiver};
use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::platform::HotkeyEvent;
use crate::platform::HWND;
use crate::state::{AppState, Layout};
use crate::telemetry::Telemetry;
use crate::ui::{WidgetFrameStyle, WIDGETS};

const FOREGROUND_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// ウィジェットのスケール基準となる「論理ポイント換算後」の縦解像度。
///
/// ウィジェットが画面に占める割合は `intrinsic * scale / WIDGET_BASELINE_HEIGHT` で一定になる
/// (物理解像度・DPI に依存しない)。値を下げるほどウィジェットは相対的に大きくなる。
///
/// 1440 は「修正前に 4K (150% DPI ≒ ppp 1.5) で表示されていたサイズ」に合わせた基準。
/// = 2160 / 1.5。4K モニタの表示スケールが 150% 以外なら `2160 / 実 DPI 倍率` に調整する。
const WIDGET_BASELINE_HEIGHT: f32 = 1440.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    /// UDP バインドアドレス。Forza 側の "Data Out IP" は 127.0.0.1、Port は同値を設定。
    bind: String,
    /// 設定 (メイン) ウィンドウの初期サイズ
    #[serde(default = "default_settings_size")]
    settings_size: [f32; 2],
    /// ウィジェット配置
    #[serde(default)]
    layout: Layout,
    /// オーバーレイ全体の ON/OFF (設定ウィンドウのトグル)
    #[serde(default = "default_true")]
    overlay_enabled: bool,
    /// `target_processes` のいずれかがフォアグラウンドのときだけ表示する
    #[serde(default = "default_true")]
    auto_hide_when_inactive: bool,
    /// ターゲットゲームの実行ファイル名 (大文字小文字無視で比較)
    #[serde(default = "default_targets")]
    target_processes: Vec<String>,
    /// GPU 選択ヒント: "auto" | "high_performance" | "low_power"
    /// (変更はアプリ再起動で有効化)
    #[serde(default = "default_gpu_pref")]
    gpu_preference: String,
    /// ACC/BRK テキスト背景の透明度 (0=完全透明..255=不透明)
    #[serde(default = "default_input_text_bg_alpha")]
    input_text_bg_alpha: u8,
    /// ACC/BRK テキスト背景の追加パディング (px, scale=1.0 基準)
    #[serde(default = "default_input_text_pad")]
    input_text_pad: f32,
    /// 速度ウィジェットの単位: true = km/h, false = mph
    #[serde(default = "default_true")]
    speed_unit_kph: bool,
    /// 横 G バーウィジェットの表示レンジ上限 (G)
    #[serde(default = "default_g_bar_max_g")]
    g_bar_max_g: f32,
    /// スリップインジケーター: 内側方向のスリップを 0 とみなすか
    #[serde(default = "default_true")]
    ignore_inward_slip: bool,
    /// 受信した生パケットを他ツールへ転送するか
    #[serde(default)]
    forward_enabled: bool,
    /// 転送先 "IP:Port" (例: 127.0.0.1:5300)
    #[serde(default = "default_forward_target")]
    forward_target: String,
}

fn default_true() -> bool {
    true
}
fn default_targets() -> Vec<String> {
    vec!["forzahorizon6.exe".to_string()]
}
fn default_settings_size() -> [f32; 2] {
    [520.0, 600.0]
}
fn default_gpu_pref() -> String {
    "high_performance".to_string()
}
fn default_input_text_bg_alpha() -> u8 {
    107
}
fn default_input_text_pad() -> f32 {
    6.0
}
fn default_forward_target() -> String {
    "127.0.0.1:5300".to_string()
}
fn default_g_bar_max_g() -> f32 {
    4.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:35530".to_string(),
            settings_size: default_settings_size(),
            layout: Layout::default(),
            overlay_enabled: true,
            auto_hide_when_inactive: true,
            target_processes: default_targets(),
            gpu_preference: default_gpu_pref(),
            input_text_bg_alpha: default_input_text_bg_alpha(),
            input_text_pad: default_input_text_pad(),
            speed_unit_kph: true,
            g_bar_max_g: default_g_bar_max_g(),
            ignore_inward_slip: true,
            forward_enabled: false,
            forward_target: default_forward_target(),
        }
    }
}

fn config_path() -> PathBuf {
    PathBuf::from("config.toml")
}

fn profiles_path() -> PathBuf {
    PathBuf::from("profiles.toml")
}

fn load_config() -> Config {
    let path = config_path();
    if let Ok(s) = std::fs::read_to_string(&path) {
        match toml::from_str::<Config>(&s) {
            Ok(c) => return c,
            Err(e) => log::warn!("config.toml parse error: {e}; using defaults"),
        }
    }
    let cfg = Config::default();
    if let Ok(s) = toml::to_string_pretty(&cfg) {
        let _ = std::fs::write(&path, s);
    }
    cfg
}

/// 既存 `config.toml` をマージしながら更新可能な項目を書き戻す。
pub fn save_config(
    path: &Path,
    layout: &Layout,
    overlay_enabled: bool,
    auto_hide: bool,
    gpu_preference: &str,
    input_text_bg_alpha: u8,
    input_text_pad: f32,
    speed_unit_kph: bool,
    g_bar_max_g: f32,
    ignore_inward_slip: bool,
    udp_port: u16,
    forward_enabled: bool,
    forward_target: &str,
) -> Result<()> {
    let mut cfg: Config = match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    };
    cfg.layout = layout.clone();
    cfg.overlay_enabled = overlay_enabled;
    cfg.auto_hide_when_inactive = auto_hide;
    cfg.gpu_preference = gpu_preference.to_string();
    cfg.input_text_bg_alpha = input_text_bg_alpha;
    cfg.input_text_pad = input_text_pad;
    cfg.speed_unit_kph = speed_unit_kph;
    cfg.g_bar_max_g = g_bar_max_g;
    cfg.ignore_inward_slip = ignore_inward_slip;
    cfg.bind = format!("0.0.0.0:{udp_port}");
    cfg.forward_enabled = forward_enabled;
    cfg.forward_target = forward_target.to_string();
    let s = toml::to_string_pretty(&cfg).context("serialize config")?;
    std::fs::write(path, s).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// 単一オーバーレイウィンドウの実行時情報。
/// 全表示中ウィジェットの外接矩形 (bbox) を 1 枚の透明ウィンドウで覆うことで
/// DWM コンポジットに乗せるレイヤー数を最小化している。
#[derive(Default)]
struct OverlayWindowState {
    /// クリックスルー (EX_STYLE) を適用済みの HWND
    clickthrough_hwnd: Option<HWND>,
}

/// オーバーレイウィンドウのタイトル (HWND 検索用)
const OVERLAY_TITLE: &str = "ArkForzaWidgets-overlay";

struct App {
    state: Arc<Mutex<AppState>>,
    rx: Receiver<Telemetry>,
    config_path: PathBuf,
    profiles_path: PathBuf,
    target_processes: Vec<String>,
    last_fg_check: Instant,
    /// 単一オーバーレイウィンドウの状態
    overlay_state: OverlayWindowState,
    /// プライマリディスプレイサイズ (1px クランプ用、とれないときは None)
    primary_display: Option<(u32, u32)>,
    /// グローバルホットキー (Alt+S / Alt+D) の受信側。起動失敗時は None。
    hotkey_rx: Option<Receiver<HotkeyEvent>>,
    /// 受信パケットの転送先 (受信スレッドと共有)
    forward: Arc<telemetry::receiver::ForwardLink>,
    /// 直近に転送リンクへ適用した設定 (変更検出用)
    last_forward: Option<(bool, String)>,
}

impl App {
    fn new(
        state: Arc<Mutex<AppState>>,
        rx: Receiver<Telemetry>,
        config_path: PathBuf,
        target_processes: Vec<String>,
        forward: Arc<telemetry::receiver::ForwardLink>,
    ) -> Self {
        let primary_display = platform::primary_display_size();
        if let Some((w, h)) = primary_display {
            log::info!("primary display = {}x{}", w, h);
        }
        let hotkey_rx = platform::spawn_hotkey_listener();
        Self {
            state,
            rx,
            config_path,
            profiles_path: profiles_path(),
            target_processes,
            last_fg_check: Instant::now() - FOREGROUND_POLL_INTERVAL,
            overlay_state: OverlayWindowState::default(),
            primary_display,
            hotkey_rx,
            forward,
            last_forward: None,
        }
    }

    fn poll_foreground(&mut self) {
        if self.last_fg_check.elapsed() < FOREGROUND_POLL_INTERVAL {
            return;
        }
        self.last_fg_check = Instant::now();

        let matched = match platform::foreground_process_name() {
            Some(name) => self
                .target_processes
                .iter()
                .any(|t| t.eq_ignore_ascii_case(&name)),
            None => false,
        };
        // ゲームがフォアグラウンドなら、そのウィンドウのクライアント領域から
        // レンダリング解像度と画面上の原点を取得する (最新の検出値を保持)。
        let game_rect = if matched {
            platform::foreground_window_rect()
        } else {
            None
        };
        let mut st = self.state.lock().unwrap();
        st.target_in_foreground = matched;
        if let Some((x, y, w, h)) = game_rect {
            st.game_origin = (x, y);
            st.game_resolution = Some((w, h));
        }
    }

    /// グローバルホットキーを処理する。
    /// Alt+S: 未保存なら現在のダイノ/ギア比をプロファイルへ保存。
    /// Alt+D: 保存済みならプロファイルを削除してライブ記録へ戻す。
    fn handle_hotkeys(&mut self) {
        let Some(rx) = self.hotkey_rx.as_ref() else {
            return;
        };
        // 溜まったイベントをすべて処理 (通常は 0〜1 件)
        let events: Vec<HotkeyEvent> = rx.try_iter().collect();
        if events.is_empty() {
            return;
        }
        let mut st = self.state.lock().unwrap();
        let mut changed = false;
        for ev in events {
            match ev {
                HotkeyEvent::SaveProfile => {
                    // 未保存かつ記録データがあるときだけ保存する。
                    if !st.has_current_profile()
                        && (st.dyno.has_data() || st.live_recorded_gear_count() > 0)
                    {
                        if let Some(key) = st.save_current_profile() {
                            log::info!("hotkey: saved profile for {key}");
                            changed = true;
                        }
                    }
                }
                HotkeyEvent::ClearProfile => {
                    // 保存済みプロファイルがあれば削除し、ライブデータも常にクリアする。
                    if st.has_current_profile() {
                        st.delete_current_profile();
                        log::info!("hotkey: cleared saved profile");
                        changed = true;
                    }
                    st.clear_live_data();
                }
            }
        }
        if changed {
            let profiles = st.profiles.clone();
            drop(st);
            match crate::state::save_profiles(&self.profiles_path, &profiles) {
                Ok(()) => log::info!("profiles saved to {}", self.profiles_path.display()),
                Err(e) => log::warn!("failed to save profiles: {e}"),
            }
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // 子ビューポート (透明) のために clear は完全透明にする。
        // メインウィンドウ (Settings) は CentralPanel が不透明背景を持つので問題ない。
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- 受信キューを排出 ---
        {
            let mut st = self.state.lock().unwrap();
            while let Ok(sample) = self.rx.try_recv() {
                st.ingest(sample);
            }
        }

        // --- 転送設定を受信スレッドへ反映 (変更時のみ) ---
        {
            let (enabled, target) = {
                let st = self.state.lock().unwrap();
                (st.forward_enabled, st.forward_target.clone())
            };
            if self.last_forward.as_ref() != Some(&(enabled, target.clone())) {
                self.forward.update(enabled, &target);
                self.last_forward = Some((enabled, target));
            }
        }

        // --- フォアグラウンド判定 (250ms ごと) ---
        self.poll_foreground();

        // --- グローバルホットキー (Alt+S = 保存 / Alt+D = クリア) ---
        self.handle_hotkeys();

        // --- メインウィンドウ = 設定パネル ---
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut st = self.state.lock().unwrap();
            ui::editor::show(ui, &mut st, &self.config_path, &self.profiles_path);
        });

        // --- 表示中ウィジェットを 1 枚の透明ウィンドウで覆う (外接矩形 = bbox) ---
        // ドライバ: DWM に乗せるレイヤー 1 枚にすると N 依存の fps 低下が消える。
        // bbox がディスプレイ全体を覆いそうなときは「ディスプレイ - 1px」にクランプして
        // Forza のフルスクリーンフリップ (Independent Flip / MPO) を奪わないようにする。
        let (should_show, layout_snapshot, resolution, origin) = {
            let st = self.state.lock().unwrap();
            // 解像度の優先順位: ゲーム検出値 > プライマリディスプレイ > 1920x1080。
            let resolution = st
                .game_resolution
                .map(|(w, h)| (w as f32, h as f32))
                .or_else(|| self.primary_display.map(|(w, h)| (w as f32, h as f32)))
                .unwrap_or((1920.0, 1080.0));
            // Settings ウィンドウがフォーカスされているときはゲームが非フォアグラウンドでも
            // オーバーレイを表示する (位置を見ながら調整できるようにするため)。
            // overlay_enabled=false だけは尊重する。
            let settings_focused = ctx.input(|i| i.focused);
            let should_show = st.should_show_overlay() || (settings_focused && st.overlay_enabled);
            (should_show, st.layout.clone(), resolution, st.game_origin)
        };

        // ゲーム解像度/原点は物理ピクセル (GetClientRect/ClientToScreen)。
        // 一方 egui のビューポート配置 (OuterPosition/InnerSize) と Area 座標は
        // 論理ポイント単位で、egui-winit が pixels_per_point を掛けて物理化する。
        //
        // プロセスは System DPI Aware (main() 冒頭で設定) なので、全ウィンドウの
        // scale_factor はシステム DPI で固定され、設定ウィンドウをどのモニタへ動かしても
        // `ctx.pixels_per_point()` は変化しない。座標系 (GetClientRect / OuterPosition 等) も
        // すべて同じシステム DPI 空間で仮想化されるため、ここは ctx の値をそのまま使えば整合する。
        let ppp = ctx.pixels_per_point().max(0.1);
        let resolution = (resolution.0 / ppp, resolution.1 / ppp);
        let origin = (origin.0 as f32 / ppp, origin.1 as f32 / ppp);

        // ウィジェットの解像度スケール: 基準 (2160) に対する縦解像度の比率。
        // 例) 2160 → 1.0、1080 → 0.5。各ウィジェットの scale にこの係数を掛ける。
        //
        // 重要: ここは **ppp 除算後の論理解像度** (中心位置の算出と同じ基準) から計算する。
        // egui はウィジェットの論理ポイントサイズに ppp を掛けて物理化するため、
        // 論理解像度から係数を作れば「ウィジェットが画面に占める割合」は ppp・物理解像度に
        // 依存せず一定になる。物理ピクセル (ppp 除算前) から計算すると、描画時の ppp 乗算と
        // 二重にかかり、4K↔FHD で倍率がズレて低解像度側が過剰に小さくなる。
        let res_scale = (resolution.1 / WIDGET_BASELINE_HEIGHT).max(0.05);

        // 表示中ウィジェットの矩形を集めて union を取る
        let mut bbox: Option<egui::Rect> = None;
        if should_show {
            for w in WIDGETS {
                let item = (w.get)(&layout_snapshot);
                if !item.visible {
                    continue;
                }
                let r = widget_screen_rect(w.intrinsic, item, resolution, origin, res_scale);
                bbox = Some(match bbox {
                    Some(prev) => prev.union(r),
                    None => r,
                });
            }
        }

        if let Some(mut bbox) = bbox {
            // 負座標はクリップ (ウィンドウを画面外に到達させるのを避ける)
            if bbox.min.x < 0.0 {
                bbox.min.x = 0.0;
            }
            if bbox.min.y < 0.0 {
                bbox.min.y = 0.0;
            }

            // 画面全体を覆いそうなときはディスプレイ - 1px にクランプ (Independent Flip 保護)
            // primary_display は物理ピクセルなので、論理ポイントの bbox に合わせて ppp で換算。
            if let Some((dw, dh)) = self.primary_display {
                let max_w = (dw as f32 / ppp - 1.0).max(1.0);
                let max_h = (dh as f32 / ppp - 1.0).max(1.0);
                if bbox.width() > max_w {
                    bbox.max.x = bbox.min.x + max_w;
                }
                if bbox.height() > max_h {
                    bbox.max.y = bbox.min.y + max_h;
                }
            }

            let bbox_pos = [bbox.min.x, bbox.min.y];
            let bbox_size = [bbox.width().max(1.0), bbox.height().max(1.0)];
            let viewport_id = egui::ViewportId::from_hash_of("arkforzawidgets_overlay");
            let builder = egui::ViewportBuilder::default()
                .with_title(OVERLAY_TITLE)
                .with_inner_size(bbox_size)
                .with_position(bbox_pos)
                .with_decorations(false)
                .with_transparent(true)
                .with_always_on_top()
                .with_resizable(false)
                .with_active(false);

            let state_for_paint = Arc::clone(&self.state);
            let bbox_min = bbox.min;
            let resolution_for_paint = resolution;
            let origin_for_paint = origin;
            let res_scale_for_paint = res_scale;

            ctx.show_viewport_immediate(viewport_id, builder, move |vctx, _class| {
                // 位置とサイズを毎フレ反映
                vctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(bbox_size.into()));
                vctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(bbox_pos.into()));

                let panel_frame = egui::Frame::none()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::ZERO);
                egui::CentralPanel::default()
                    .frame(panel_frame)
                    .show(vctx, |_ui| {
                        let st = state_for_paint.lock().unwrap();
                        for w in WIDGETS {
                            let item = (w.get)(&st.layout);
                            if !item.visible {
                                continue;
                            }
                            let scale_x = item.scale[0].max(0.1) * res_scale_for_paint;
                            let scale_y = item.scale[1].max(0.1) * res_scale_for_paint;
                            let scale_vec = egui::Vec2::new(scale_x, scale_y);
                            let s_min = scale_x.min(scale_y);

                            let frame = match w.frame_style {
                                WidgetFrameStyle::Boxed => egui::Frame::none()
                                    .fill(egui::Color32::from_black_alpha(140))
                                    .rounding(6.0 * s_min)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_white_alpha(40),
                                    ))
                                    .inner_margin(egui::Margin::same(8.0 * s_min)),
                                WidgetFrameStyle::Transparent => egui::Frame::none()
                                    .fill(egui::Color32::TRANSPARENT)
                                    .inner_margin(egui::Margin::ZERO),
                            };

                            // 重心百分率 -> スクリーン矩形 -> bbox ウィンドウ内の相対座標。
                            let rect = widget_screen_rect(
                                w.intrinsic,
                                item,
                                resolution_for_paint,
                                origin_for_paint,
                                res_scale_for_paint,
                            );
                            let rel = egui::pos2(rect.min.x - bbox_min.x, rect.min.y - bbox_min.y);
                            let paint_fn = w.paint;
                            let st_ref = &*st;
                            egui::Area::new(egui::Id::new(("arkforzawidgets_widget_area", w.id)))
                                .fixed_pos(rel)
                                .order(egui::Order::Foreground)
                                .interactable(false)
                                .show(vctx, |ui| {
                                    frame.show(ui, |ui| {
                                        paint_fn(ui, st_ref, scale_vec);
                                    });
                                });
                        }
                    });
            });

            // オーバーレイ HWND にクリックスルー EX_STYLE を適用 (HWND が変わったときだけ)
            if let Some(hwnd) = platform::find_hwnd_by_title(OVERLAY_TITLE) {
                let need_apply = match self.overlay_state.clickthrough_hwnd {
                    Some(prev) if prev.0 == hwnd.0 => false,
                    _ => true,
                };
                if need_apply && platform::apply_clickthrough_hwnd(hwnd) {
                    self.overlay_state.clickthrough_hwnd = Some(hwnd);
                }
            }
        } else {
            // 非表示: ビューポート破棄。HWND キャッシュも破棄。
            self.overlay_state.clickthrough_hwnd = None;
        }

        // イベント駆動再描画:
        // - UDP 受信時は受信スレッドが `request_repaint()` を呼ぶ (モニタリフレッシュレートで描画)
        // - フォアグラウンド検出 / Forza 終了の検知のため、低頻度 (250ms) で再描画を保証
        ctx.request_repaint_after(FOREGROUND_POLL_INTERVAL);
    }
}

/// ウィジェットの「重心 (中心) 百分率」配置からスクリーン上の矩形を計算する。
///
/// `item.pos` はゲーム解像度に対する中心位置の比率 (0.0..=1.0)。
/// `resolution` / `origin` はいずれも **論理ポイント** (DPI 換算済み)。
/// `res_scale` はゲーム縦解像度 (2160 基準) に応じたサイズ倍率。
fn widget_screen_rect(
    intrinsic: egui::Vec2,
    item: &state::LayoutItem,
    resolution: (f32, f32),
    origin: (f32, f32),
    res_scale: f32,
) -> egui::Rect {
    let sx = item.scale[0].max(0.1) * res_scale;
    let sy = item.scale[1].max(0.1) * res_scale;
    let size = egui::vec2(intrinsic.x * sx, intrinsic.y * sy);
    let fx = item.pos[0].clamp(0.0, 1.0);
    let fy = item.pos[1].clamp(0.0, 1.0);
    let center = egui::pos2(origin.0 + fx * resolution.0, origin.1 + fy * resolution.1);
    egui::Rect::from_center_size(center, size)
}

/// `gpu_preference` 文字列を `wgpu::PowerPreference` に変換。
fn parse_gpu_preference(s: &str) -> eframe::wgpu::PowerPreference {
    use eframe::wgpu::PowerPreference;
    match s.trim().to_ascii_lowercase().as_str() {
        "low_power" | "low" | "integrated" => PowerPreference::LowPower,
        "high_performance" | "high" | "discrete" => PowerPreference::HighPerformance,
        _ => PowerPreference::None, // "auto"
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // ウィンドウ生成より前にプロセスを System DPI Aware へ固定する。
    // これで全ウィンドウの scale_factor がシステム DPI に固定され、設定ウィンドウを
    // 別倍率モニタへ動かしても egui の pixels_per_point が変化しない。
    // (異なる ppp が共存するとフォントアトラス共有でテクスチャ境界パニックが起きる)
    platform::set_system_dpi_aware();

    let cfg = load_config();
    let cfg_path = config_path();

    // 容量 1 = 最新値だけ保持 (古いサンプルは捨てる)
    let (tx, rx) = bounded::<Telemetry>(1);

    // メインウィンドウ = 設定ウィンドウ (装飾あり、リサイズ可、最前面ではない)
    //
    // 注意: `with_transparent(true)` をここで指定するのは **必須**。
    // eframe 0.29 の wgpu バックエンドは「メインビューポートの transparent フラグ」を見て
    // wgpu サーフェスの `CompositeAlphaMode` を決定しており、子ビューポート個別の
    // `with_transparent(true)` は無視される。これを付けないと子ビューポートの背景が
    // 不透明黒で塗りつぶされる。
    // Settings ウィンドウ自体は `CentralPanel` が `panel_fill` (不透明) で塗るので表示は変わらない。
    let viewport = egui::ViewportBuilder::default()
        .with_title("ArkForzaWidgets - Settings")
        .with_inner_size(cfg.settings_size)
        .with_min_inner_size([360.0, 320.0])
        .with_resizable(true)
        .with_transparent(true);

    // Present mode は NoVsync (低遅延)。
    // Settings ウィンドウが暴走しないための仕掛け:
    // - update() 末尾の `request_repaint_after` は 250ms (アイドル時は 4Hz)
    // - UDP 受信スレッドが受信時に `request_repaint()` を呼ぶ → Forza データレート上限で描画
    // これにより NoVsync でも実質フレームレートは UDP レート (通常 60Hz) 以下に収まる。
    // GPU 選択は config の `gpu_preference` で制御 (デフォルト: auto)。
    let power_preference = parse_gpu_preference(&cfg.gpu_preference);
    log::info!(
        "wgpu power_preference = {:?} (config: {:?})",
        power_preference,
        cfg.gpu_preference
    );
    let wgpu_options = eframe::egui_wgpu::WgpuConfiguration {
        present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
        desired_maximum_frame_latency: Some(1),
        power_preference,
        ..Default::default()
    };

    let native = eframe::NativeOptions {
        viewport,
        vsync: false,
        wgpu_options,
        ..Default::default()
    };

    let bind = cfg.bind.clone();
    let target_processes = cfg.target_processes.clone();
    let initial_state = {
        let mut s = AppState::default();
        s.layout = cfg.layout.clone();
        s.overlay_enabled = cfg.overlay_enabled;
        s.auto_hide_when_inactive = cfg.auto_hide_when_inactive;
        s.gpu_preference = cfg.gpu_preference.clone();
        s.input_text_bg_alpha = cfg.input_text_bg_alpha;
        s.input_text_pad = cfg.input_text_pad;
        s.speed_unit_kph = cfg.speed_unit_kph;
        s.g_bar_max_g = cfg.g_bar_max_g;
        s.ignore_inward_slip = cfg.ignore_inward_slip;
        s.udp_port = cfg
            .bind
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(35530);
        s.forward_enabled = cfg.forward_enabled;
        s.forward_target = cfg.forward_target.clone();
        s.profiles = crate::state::load_profiles(&profiles_path());
        Arc::new(Mutex::new(s))
    };

    // 受信スレッドと共有する転送リンク。起動時の設定を反映。
    let forward = Arc::new(telemetry::receiver::ForwardLink::new());
    forward.update(cfg.forward_enabled, &cfg.forward_target);

    eframe::run_native(
        "ArkForzaWidgets",
        native,
        Box::new(move |cc| {
            // カスタムフォント (Montserrat Italic) を登録
            ui::fonts::install(&cc.egui_ctx);

            // 起動時に選ばれた wgpu アダプタ情報を取得して AppState に保存
            if let Some(rs) = cc.wgpu_render_state.as_ref() {
                let info = rs.adapter.get_info();
                let desc = format!(
                    "{} ({:?} / {:?})",
                    info.name, info.device_type, info.backend
                );
                log::info!("wgpu adapter: {desc}");
                if let Ok(mut st) = initial_state.lock() {
                    st.active_gpu = desc;
                }
            }

            telemetry::receiver::spawn(&bind, tx, cc.egui_ctx.clone(), forward.clone())
                .expect("failed to spawn telemetry receiver");
            Ok(Box::new(App::new(
                initial_state,
                rx,
                cfg_path,
                target_processes,
                forward,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}
