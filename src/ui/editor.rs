//! 設定パネル (メインウィンドウの中身)。
//!
//! ウィジェット (Inputs/Dyno/...) はすべて子ビューポート (透明・クリックスルー) として
//! 別ウィンドウに描画されるため、ここは普通の装飾付きウィンドウの中身として
//! `ui` を受け取って描画するだけのシンプルな関数。

use std::path::Path;

use egui::{Color32, Ui};

use crate::state::{AppState, Layout};
use crate::ui::{WidgetSpec, WIDGETS};

/// 設定パネル本体を描画。
///
/// `state` を可変借用するので、呼び出し側は `AppState` のロックを取った
/// `MutexGuard` を渡す。
///
/// レイアウト:
/// - 1 段目 (内容ぶんの最小高さ): 左 = Overlay、右 = GPU
/// - 2 段目: Car Profile (左列 = Car Ordinal..Rev limit、右列 = ギア比)
/// - 3 段目以降 (残り全部): Widgets (ヘッダ右端に Save/Reset、2 列グリッド、あふれたらスクロール)
pub fn show(ui: &mut Ui, state: &mut AppState, config_path: &Path) {
    // 設定ウィンドウがフォーカスされていないとき (= ゲームをプレイ中で設定ウィンドウが非フォーカス) は、
    // 重い mini Dyno グラフの再構築をスキップして 1 フレームのコストを下げる。
    let focused = ui.ctx().input(|i| i.focused);

    // --- 1 段目: 左 = Overlay、右 = GPU ---
    ui.columns(2, |cols| {
        overlay_section(&mut cols[0], state);
        gpu_section(&mut cols[1], state);
    });

    ui.add_space(6.0);
    ui.separator();

    // --- 2 段目: Car Profile ---
    car_profile_panel(ui, state, focused);

    ui.add_space(6.0);
    ui.separator();

    // --- 3 段目以降: Widgets (ヘッダ行右端に Save/Reset) ---
    ui.horizontal(|ui| {
        ui.heading("Widgets");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Reset").clicked() {
                state.layout = Layout::default();
            }
            if ui.button("💾 Save to config.toml").clicked() {
                match crate::save_config(
                    config_path,
                    &state.layout,
                    state.overlay_enabled,
                    state.auto_hide_when_inactive,
                    &state.gpu_preference,
                    state.input_text_bg_alpha,
                    state.input_text_pad,
                    state.speed_unit_kph,
                    state.g_bar_max_g,
                    state.ignore_inward_slip,
                    state.udp_port,
                    state.forward_enabled,
                    &state.forward_target,
                ) {
                    Ok(()) => log::info!("config saved to {}", config_path.display()),
                    Err(e) => log::warn!("failed to save config: {e}"),
                }
            }
        });
    });
    ui.add_space(4.0);

    // ウィンドウからあふれたらこの一覧だけスクロール可能にする。
    let spacing = 12.0;
    let col_width = ((ui.available_width() - spacing) * 0.5).max(120.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("widgets_grid")
                .num_columns(2)
                .spacing(egui::vec2(spacing, 8.0))
                .show(ui, |ui| {
                    for (i, w) in WIDGETS.iter().enumerate() {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_width(col_width);
                            widget_row(ui, w, state);
                        });
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
        });
}

fn overlay_section(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("Overlay");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Minimize").clicked() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        });
    });
    ui.checkbox(&mut state.overlay_enabled, "Show overlay");
    ui.checkbox(
        &mut state.auto_hide_when_inactive,
        "Auto-hide when game is not in foreground",
    );
    ui.horizontal(|ui| {
        ui.label("Speed unit:");
        ui.selectable_value(&mut state.speed_unit_kph, true, "km/h");
        ui.selectable_value(&mut state.speed_unit_kph, false, "mph");
    });
    let status = if state.target_in_foreground {
        "● game detected in foreground"
    } else {
        "○ game not in foreground"
    };
    ui.label(
        egui::RichText::new(status)
            .size(11.0)
            .color(if state.target_in_foreground {
                Color32::from_rgb(120, 220, 120)
            } else {
                Color32::from_white_alpha(140)
            }),
    );

    // 検出したゲーム解像度 (ウィジェット位置の基準)。
    let res_text = match state.game_resolution {
        Some((w, h)) => format!("Game resolution: {w}×{h}"),
        None => "Game resolution: (not detected — focus the game once)".to_string(),
    };
    ui.label(
        egui::RichText::new(res_text)
            .size(11.0)
            .color(if state.game_resolution.is_some() {
                Color32::from_white_alpha(200)
            } else {
                Color32::from_white_alpha(140)
            }),
    );
}

fn gpu_section(ui: &mut Ui, state: &mut AppState) {
    ui.heading("GPU");
    ui.label(
        egui::RichText::new(format!(
            "Active: {}",
            if state.active_gpu.is_empty() {
                "(unknown)"
            } else {
                state.active_gpu.as_str()
            }
        ))
        .size(11.0)
        .color(Color32::from_white_alpha(180)),
    );
    ui.horizontal(|ui| {
        ui.label("Preference:");
        egui::ComboBox::from_id_source("gpu_pref_combo")
            .selected_text(state.gpu_preference.as_str())
            .show_ui(ui, |ui| {
                for opt in ["auto", "high_performance", "low_power"] {
                    ui.selectable_value(&mut state.gpu_preference, opt.to_string(), opt);
                }
            });
    });
    ui.label(
        egui::RichText::new("Save & restart the app to apply GPU change.")
            .size(10.0)
            .color(Color32::from_white_alpha(140)),
    );

    ui.add_space(8.0);
    ui.heading("UDP");
    ui.horizontal(|ui| {
        ui.label("Receive port:");
        ui.add(egui::DragValue::new(&mut state.udp_port).range(1024..=65535));
    });
    ui.label(
        egui::RichText::new("Match Forza \"Data Out IP Port\". Save & restart to apply.")
            .size(10.0)
            .color(Color32::from_white_alpha(140)),
    );

    ui.add_space(6.0);
    ui.checkbox(
        &mut state.forward_enabled,
        "Forward packets to another tool",
    );
    ui.horizontal(|ui| {
        ui.label("Destination:");
        ui.add_enabled(
            state.forward_enabled,
            egui::TextEdit::singleline(&mut state.forward_target)
                .hint_text("127.0.0.1:5300")
                .desired_width(140.0),
        );
    });
    // 転送先が不正なら警告を出す (有効時のみ)。
    if state.forward_enabled
        && state
            .forward_target
            .trim()
            .parse::<std::net::SocketAddr>()
            .is_err()
    {
        ui.label(
            egui::RichText::new("Invalid address. Use IP:Port (e.g. 127.0.0.1:5300).")
                .size(10.0)
                .color(Color32::from_rgb(230, 140, 120)),
        );
    } else {
        ui.label(
            egui::RichText::new("Mirrors received packets live (no restart needed).")
                .size(10.0)
                .color(Color32::from_white_alpha(140)),
        );
    }
}

/// 右半分: 現在の車 (CarOrdinal / PI) ごとの Dyno/パワーバンドプロファイル管理。
fn car_profile_panel(ui: &mut Ui, state: &mut AppState, focused: bool) {
    ui.heading("Car Profile");
    ui.add_space(4.0);

    let key = state.current_car_key();
    let Some(key) = key else {
        ui.label(
            egui::RichText::new("Waiting for car telemetry…\n(start a race to detect the car)")
                .size(11.0)
                .color(Color32::from_white_alpha(150)),
        );
        return;
    };

    egui::Frame::group(ui.style()).show(ui, |ui| {
        // パワーバンドのハイライト閾値は固定 95% (以前は Settings でスライダー編集可能だった)
        const BAND_RATIO: f32 = 0.95;

        ui.columns(2, |cols| {
            // ===== 左列: Car Ordinal .. Rev limit =====
            let ui = &mut cols[0];
            ui.label(format!("Car Ordinal: {}", state.last_car_ordinal));
            ui.label(format!("Performance Index: {}", state.last_car_pi));
            ui.label(
                egui::RichText::new(format!("key: {key}"))
                    .size(10.0)
                    .color(Color32::from_white_alpha(120)),
            );

            // --- 簡易 Dyno グラフ (線・パワーバンド・レブリミットのみ) ---
            let (power, torque, band) = if let Some(p) = state.profiles.get(&key) {
                (
                    p.power_series(),
                    p.torque_series(),
                    p.power_band_with(BAND_RATIO),
                )
            } else {
                (
                    state.dyno.power_series(),
                    state.dyno.torque_series(),
                    state.dyno.power_band(BAND_RATIO),
                )
            };
            let x_max = if state.latest.engine_max_rpm > 0.0 {
                state.latest.engine_max_rpm as f64
            } else if let Some(p) = state.profiles.get(&key) {
                p.max_rpm as f64
            } else {
                state.dyno.max_rpm as f64
            };
            mini_dyno(
                ui,
                &power,
                &torque,
                band,
                rev_limit_for_view(state, &key),
                x_max,
                focused,
            );

            // レブリミット (グラフ外に別項目で表示)
            match rev_limit_for_view(state, &key) {
                Some(limit) => ui.label(
                    egui::RichText::new(format!("Rev limit: {:.0} rpm", limit))
                        .size(11.0)
                        .color(Color32::from_rgb(255, 120, 120)),
                ),
                None => ui.label(
                    egui::RichText::new("Rev limit: (not detected yet)")
                        .size(11.0)
                        .color(Color32::from_white_alpha(140)),
                ),
            };

            // ギア比の記録状況 (保存済みプロファイルがあればそちら、無ければライブ記録)
            let gear_count = state
                .profiles
                .get(&key)
                .map(|p| p.recorded_gear_count())
                .filter(|&n| n > 0)
                .unwrap_or_else(|| state.live_recorded_gear_count());
            ui.label(
                egui::RichText::new(format!("Gear ratios: {} recorded", gear_count))
                    .size(11.0)
                    .color(Color32::from_white_alpha(160)),
            );

            // ===== 右列: ギアごとの減速比 (rpm/車速) と最適シフト RPM =====
            let ui = &mut cols[1];
            ui.label(
                egui::RichText::new(format!("Gear ratios ({})", state.gear_ratio_unit_suffix()))
                    .size(12.0)
                    .color(Color32::from_white_alpha(200)),
            );
            ui.add_space(2.0);
            let rev_limit = rev_limit_for_view(state, &key);
            // 保存済みプロファイルがあればそちら、無ければライブ記録の値を表示。
            let saved_profile = state.profiles.get(&key);
            let mut any = false;
            for g in 1u8..11 {
                let ratio = match saved_profile {
                    Some(p) => p.gear_ratio(g),
                    None => state.live_gear_ratio(g),
                };
                let Some(ratio) = ratio else { continue };
                any = true;
                let ratio = state.display_gear_ratio(ratio);
                // 次ギアへの最適シフト RPM (計算できれば添える)
                let shift = saved_profile
                    .zip(rev_limit)
                    .and_then(|(p, rl)| p.optimal_shift_rpm(g, rl));
                let text = match shift {
                    Some(rpm) => format!("G{g}: {ratio:.2}  ⇧{rpm:.0}"),
                    None => format!("G{g}: {ratio:.2}"),
                };
                ui.label(egui::RichText::new(text).size(11.0).monospace());
            }
            if !any {
                ui.label(
                    egui::RichText::new("(not recorded yet)")
                        .size(11.0)
                        .color(Color32::from_white_alpha(130)),
                );
            }
        });
    });
}

/// 設定パネル用の簡易 Dyno グラフ。線・パワーバンド帯・レブリミット線のみ。
///
/// `focused == false` (ゲームをプレイ中で設定ウィンドウが非フォーカス) のときは
/// 高コストな Plot 構築をスキップし、軽量なプレースホルダだけ描く。
fn mini_dyno(
    ui: &mut Ui,
    power: &[[f64; 2]],
    torque: &[[f64; 2]],
    band: Option<(f32, f32)>,
    rev_limit: Option<f32>,
    x_max: f64,
    focused: bool,
) {
    use egui_plot::{Line, Plot, PlotPoints, Polygon, VLine};

    if !focused {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 120.0),
            egui::Sense::hover(),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "(dyno preview paused — focus window)",
            egui::FontId::proportional(11.0),
            Color32::from_white_alpha(120),
        );
        return;
    }

    let data_max = power
        .iter()
        .map(|p| p[1])
        .fold(0.0_f64, f64::max)
        .max(torque.iter().map(|p| p[1]).fold(0.0_f64, f64::max));
    let y_max = if data_max > 0.0 {
        data_max * 1.1
    } else {
        100.0
    };

    Plot::new("mini_dyno_plot")
        .height(120.0)
        .show_background(false)
        .show_axes([false, false])
        .show_grid([false, false])
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show_x(false)
        .show_y(false)
        .show(ui, |plot_ui| {
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [0.0, 0.0],
                [x_max, y_max],
            ));
            if let Some((bs, be)) = band {
                let pts = vec![
                    [bs as f64, 0.0],
                    [be as f64, 0.0],
                    [be as f64, y_max],
                    [bs as f64, y_max],
                ];
                plot_ui.polygon(
                    Polygon::new(PlotPoints::from(pts))
                        .fill_color(Color32::from_rgba_unmultiplied(255, 200, 0, 40))
                        .stroke(egui::Stroke::NONE),
                );
            }
            plot_ui.line(
                Line::new(PlotPoints::from(power.to_vec()))
                    .color(Color32::from_rgb(255, 160, 60))
                    .width(1.5),
            );
            plot_ui.line(
                Line::new(PlotPoints::from(torque.to_vec()))
                    .color(Color32::from_rgb(120, 200, 255))
                    .width(1.5),
            );
            if let Some(limit) = rev_limit {
                let color = Color32::from_rgba_unmultiplied(255, 80, 80, 220);
                plot_ui.vline(
                    VLine::new(limit as f64)
                        .color(color)
                        .stroke(egui::Stroke::new(1.5, color)),
                );
            }
        });
}

/// 表示用レブリミット: 保存済みプロファイルがあればその値、なければライブ推定値。
fn rev_limit_for_view(state: &AppState, key: &str) -> Option<f32> {
    match state.profiles.get(key) {
        Some(p) => p.rev_limit,
        None => state.rev_limit,
    }
}

fn widget_row(ui: &mut Ui, meta: &WidgetSpec, state: &mut crate::state::AppState) {
    let item = (meta.get_mut)(&mut state.layout);
    // Grid のセルは左右レイアウトを継承するため、明示的に縦並びにする。
    ui.vertical(|ui| {
        ui.checkbox(&mut item.visible, meta.label);
        ui.horizontal(|ui| {
            ui.label("scale X");
            ui.add(egui::Slider::new(&mut item.scale[0], 0.4..=10.0).fixed_decimals(2));
        });
        ui.horizontal(|ui| {
            ui.label("scale Y");
            ui.add(egui::Slider::new(&mut item.scale[1], 0.4..=10.0).fixed_decimals(2));
        });
        ui.horizontal(|ui| {
            ui.label("x");
            ui.add(
                egui::DragValue::new(&mut item.pos[0])
                    .speed(0.001)
                    .range(0.0..=1.0)
                    .fixed_decimals(3),
            );
            ui.label("y");
            ui.add(
                egui::DragValue::new(&mut item.pos[1])
                    .speed(0.001)
                    .range(0.0..=1.0)
                    .fixed_decimals(3),
            );
            ui.label(format!(
                "  ({}x{} px)",
                (meta.intrinsic.x * item.scale[0]) as i32,
                (meta.intrinsic.y * item.scale[1]) as i32
            ));
        });
        ui.label(
            egui::RichText::new("x / y = center position (0.0..1.0 of game resolution)")
                .size(10.0)
                .color(egui::Color32::from_white_alpha(120)),
        );
        // ACC/BRK テキストウィジェット共通: 背景透明度スライダー
        if meta.id == "acc_text" || meta.id == "brk_text" || meta.id == "gear_display" {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Background alpha");
                let mut alpha_f = state.input_text_bg_alpha as f32;
                if ui
                    .add(egui::Slider::new(&mut alpha_f, 0.0..=255.0).fixed_decimals(0))
                    .changed()
                {
                    state.input_text_bg_alpha = alpha_f as u8;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Padding");
                ui.add(
                    egui::Slider::new(&mut state.input_text_pad, 0.0..=40.0)
                        .fixed_decimals(1)
                        .suffix(" px"),
                );
            });
        }
        // Speed ウィジェット: 背景透明度/パディング (単位は Overlay セクションで全体設定)
        if meta.id == "speed_display" {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Background alpha");
                let mut alpha_f = state.input_text_bg_alpha as f32;
                if ui
                    .add(egui::Slider::new(&mut alpha_f, 0.0..=255.0).fixed_decimals(0))
                    .changed()
                {
                    state.input_text_bg_alpha = alpha_f as u8;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Padding");
                ui.add(
                    egui::Slider::new(&mut state.input_text_pad, 0.0..=40.0)
                        .fixed_decimals(1)
                        .suffix(" px"),
                );
            });
        }
        // 横 G バーウィジェット: 表示レンジ (最大 G)
        if meta.id == "g_bar" {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Max G");
                ui.add(
                    egui::Slider::new(&mut state.g_bar_max_g, 1.0..=8.0)
                        .fixed_decimals(1)
                        .suffix(" G"),
                );
            });
        }
        // スリップインジケーター: 内側方向スリップを無視するオプション
        if meta.id == "slip_front" || meta.id == "slip_rear" {
            ui.add_space(2.0);
            ui.checkbox(
                &mut state.ignore_inward_slip,
                "Ignore inward-direction slip (L: + only, R: - only)",
            );
        }
    });
}
