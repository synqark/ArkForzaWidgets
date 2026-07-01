//! Stats ウィジェット。
//!
//! 1 枚に以下をまとめて表示する:
//!   - 左半分: 簡易ダイノグラフ (数字・軸なし、線 + パワーバンド帯 + レブリミット線 + 現在 RPM 線)
//!   - 右上: ギアごとの減速比 (rpm/車速)。最大 11 速、1 行 6 ギア × 2 行。
//!   - 右下: 現在の車のプロファイル保存ステータスと操作ヒント。
//!       - 保存済み: 「Alt+D: Clear」
//!       - 未保存:   「Alt+S: Save」
//!
//! 実際の保存/クリアは `Alt+S` / `Alt+D` のグローバルホットキーで `main.rs` 側が行う。
//! ここは状態の表示だけを担当する (`AppState` を読むのみ)。

use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

/// scale = 1.0 のウィジェットサイズ (縦長 3 段積み)
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(380.0, 440.0);

const POWER_COLOR: Color32 = Color32::from_rgb(255, 160, 60);
const TORQUE_COLOR: Color32 = Color32::from_rgb(120, 200, 255);

pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let size = Vec2::new(INTRINSIC_SIZE.x * scale.x, INTRINSIC_SIZE.y * scale.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let s = scale.x.min(scale.y).max(0.1);
    let pad = 8.0 * s;

    let painter = ui.painter();
    // 可読性のための半透明背景
    painter.rect_filled(
        rect,
        Rounding::same(6.0 * s),
        Color32::from_rgba_unmultiplied(0, 0, 0, 150),
    );

    // --- 縦 3 段分割 (ダイノ 40% / ギア比 40% / プロファイル 20%) ---
    let content = Rect::from_min_max(
        Pos2::new(rect.left() + pad, rect.top() + pad),
        Pos2::new(rect.right() - pad, rect.bottom() - pad),
    );
    let gap = pad * 0.5;
    let avail_h = content.height() - gap * 2.0;
    let dyno_h = avail_h * 0.40;
    let gears_h = avail_h * 0.40;
    // status_h は残り (= 20%)

    let dyno_rect = Rect::from_min_max(
        content.min,
        Pos2::new(content.right(), content.top() + dyno_h),
    );
    let gears_rect = Rect::from_min_max(
        Pos2::new(content.left(), dyno_rect.bottom() + gap),
        Pos2::new(content.right(), dyno_rect.bottom() + gap + gears_h),
    );
    let status_rect = Rect::from_min_max(
        Pos2::new(content.left(), gears_rect.bottom() + gap),
        content.max,
    );

    draw_mini_dyno(ui, dyno_rect, state, s);
    draw_gears(ui, gears_rect, state, s);
    draw_status(ui, status_rect, state, s);
}

/// 左半分: 数字・軸なしの簡易ダイノ。
fn draw_mini_dyno(ui: &Ui, rect: Rect, state: &AppState, s: f32) {
    let painter = ui.painter();

    // 保存済みプロファイルがあればそれを、無ければライブのダイノバッファを使う。
    let profile = state.current_profile();
    let (power, torque, band) = if let Some(p) = profile {
        (p.power_series(), p.torque_series(), p.power_band())
    } else {
        (
            state.dyno.power_series(),
            state.dyno.torque_series(),
            state.dyno.power_band(state.band_ratio),
        )
    };
    let rev_limit = match profile {
        Some(p) => p.rev_limit,
        None => state.rev_limit,
    };

    // X 軸範囲: 0 .. EngineMaxRpm (取れなければ保存値/バッファ上限)
    let fallback_max = profile.map(|p| p.max_rpm).unwrap_or(state.dyno.max_rpm);
    let x_max = if state.latest.engine_max_rpm > 0.0 {
        state.latest.engine_max_rpm as f64
    } else {
        fallback_max as f64
    }
    .max(1.0);

    // Y 軸範囲: 0 .. max*1.1 (データ無しは下限 100)
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

    // (rpm, value) -> 矩形内ピクセル
    let to_px = |rpm: f64, val: f64| -> Pos2 {
        let tx = (rpm / x_max).clamp(0.0, 1.0) as f32;
        let ty = (val / y_max).clamp(0.0, 1.0) as f32;
        Pos2::new(
            rect.left() + tx * rect.width(),
            rect.bottom() - ty * rect.height(),
        )
    };

    // 枠
    painter.rect_stroke(
        rect,
        Rounding::ZERO,
        Stroke::new(1.0, Color32::from_white_alpha(50)),
    );

    // パワーバンド帯
    if let Some((bs, be)) = band {
        let top_left = to_px(bs as f64, y_max);
        let bottom_right = to_px(be as f64, 0.0);
        painter.rect_filled(
            Rect::from_min_max(top_left, bottom_right),
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(255, 200, 0, 40),
        );
    }

    // 線
    draw_series(painter, &power, &to_px, POWER_COLOR, (1.5 * s).max(1.0));
    draw_series(painter, &torque, &to_px, TORQUE_COLOR, (1.5 * s).max(1.0));

    // レブリミット線 (赤)
    if let Some(limit) = rev_limit {
        if (limit as f64) <= x_max {
            let x = to_px(limit as f64, 0.0).x;
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(
                    (1.5 * s).max(1.0),
                    Color32::from_rgba_unmultiplied(255, 80, 80, 220),
                ),
            );
        }
    }

    // 現在 RPM 線 (白)
    let rpm = state.latest.current_rpm;
    if rpm > 0.0 {
        let x = to_px(rpm as f64, 0.0).x;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, Color32::from_white_alpha(200)),
        );
    }

    // 凡例 (左上に小さく Power/Torque の色)
    let legend_font = FontId::new(10.0 * s, FontFamily::Name(fonts::MONTSERRAT.into()));
    painter.text(
        Pos2::new(rect.left() + 3.0 * s, rect.top() + 2.0 * s),
        Align2::LEFT_TOP,
        "PWR",
        legend_font.clone(),
        POWER_COLOR,
    );
    painter.text(
        Pos2::new(rect.left() + 32.0 * s, rect.top() + 2.0 * s),
        Align2::LEFT_TOP,
        "TRQ",
        legend_font,
        TORQUE_COLOR,
    );
}

/// (rpm, value) 点列を折れ線で描く。
fn draw_series(
    painter: &egui::Painter,
    series: &[[f64; 2]],
    to_px: &dyn Fn(f64, f64) -> Pos2,
    color: Color32,
    width: f32,
) {
    if series.len() < 2 {
        return;
    }
    for w in series.windows(2) {
        let a = to_px(w[0][0], w[0][1]);
        let b = to_px(w[1][0], w[1][1]);
        painter.line_segment([a, b], Stroke::new(width, color));
    }
}

/// ギアごとの減速比 (rpm/車速)。最大 10 速、1 行 5 ギア × 2 行。
/// 記録済みギアのみ角丸矩形で描画。セル間に 5% の隙間。未記録は非表示。
fn draw_gears(ui: &Ui, rect: Rect, state: &AppState, s: f32) {
    let painter = ui.painter();

    // タイトル行 (基本 17.5*s を 3 段 (×1.15^3 ≒ ×1.52) 拡大)
    let title_font_size = 17.5 * 1.520_9 * s;
    let title_h = title_font_size * 1.3;
    let title_font = FontId::new(title_font_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        "GEAR RATIOS",
        title_font,
        Color32::from_white_alpha(180),
    );

    // セルグリッド (5 列 × 2 行 = 最大 10 速)
    const COLS: usize = 5;
    const ROWS: usize = 2;
    let grid_top = rect.top() + title_h;
    let cell_w = rect.width() / COLS as f32;
    let cell_h = ((rect.bottom() - grid_top) / ROWS as f32).max(1.0);

    // セル間隔: 各辺に 2.5% マージン → 隣接セル間で 5% の隙間
    let mgx = cell_w * 0.025;
    let mgy = cell_h * 0.025;
    let inner_w = cell_w - mgx * 2.0;
    let inner_h = cell_h - mgy * 2.0;

    // セル内テキストサイズ: 内部の幅・高さ両方に収まる最大サイズを採る。
    //   高さ基準: 内部高さからギア番号ラベル分 (約 45%) を引いた残り
    //   幅基準:   "0.0" が内部幅の約 80% に収まるサイズ
    let cell_pad = 2.0 * s;
    let h_based = (inner_h * 0.55 - cell_pad).max(6.0 * s);
    let probe_font = FontId::new(10.0, FontFamily::Name(fonts::MONTSERRAT.into()));
    let probe_w = ui
        .fonts(|f| {
            f.layout_no_wrap("0.0".to_string(), probe_font, Color32::WHITE)
                .size()
                .x
        })
        .max(1.0);
    let w_based = (inner_w * 0.80) / probe_w * 10.0;
    let text_size = h_based.min(w_based).max(6.0 * s);
    let value_font = FontId::new(text_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    let gear_label_size = (text_size * 0.50).max(6.0 * s);
    let gear_label_font = FontId::new(gear_label_size, FontFamily::Name(fonts::MONTSERRAT.into()));

    let current_gear = state.latest.gear;
    let profile = state.current_profile();

    let mut any = false;
    for g in 1u8..=10 {
        let ratio = match profile {
            Some(p) => p.gear_ratio(g),
            None => state.live_gear_ratio(g),
        };

        // 未記録ギアは矩形・テキストとも非表示
        let Some(ratio) = ratio else { continue };
        any = true;

        let col = (g as usize - 1) % COLS;
        let row = (g as usize - 1) / COLS;
        let cell_rect = Rect::from_min_size(
            Pos2::new(
                rect.left() + col as f32 * cell_w + mgx,
                grid_top + row as f32 * cell_h + mgy,
            ),
            Vec2::new(inner_w, inner_h),
        );

        let is_cur = g == current_gear;
        let frame_color = if is_cur {
            Color32::from_rgb(255, 230, 80)
        } else {
            Color32::from_white_alpha(140)
        };
        // 角丸矩形
        painter.rect_stroke(
            cell_rect,
            Rounding::same(3.0 * s),
            Stroke::new(1.0, frame_color),
        );

        // セル左上: ギア番号ラベル (小)
        painter.text(
            Pos2::new(cell_rect.left() + cell_pad, cell_rect.top() + 1.0 * s),
            Align2::LEFT_TOP,
            format!("G{g}"),
            gear_label_font.clone(),
            if is_cur {
                Color32::from_rgb(255, 230, 80)
            } else {
                Color32::from_white_alpha(150)
            },
        );

        // セル中央: 比率値を大きく
        painter.text(
            cell_rect.center(),
            Align2::CENTER_CENTER,
            format!("{ratio:.1}"),
            value_font.clone(),
            if is_cur {
                Color32::from_rgb(255, 240, 140)
            } else {
                Color32::WHITE
            },
        );
    }

    if !any {
        let small_font = FontId::new(10.0 * s, FontFamily::Name(fonts::MONTSERRAT.into()));
        painter.text(
            Pos2::new(rect.left(), grid_top + cell_h * 0.5),
            Align2::LEFT_CENTER,
            "(not recorded yet)",
            small_font,
            Color32::from_white_alpha(130),
        );
    }
}

/// プロファイル保存ステータス (CarID / PI / Status バッジ)。
///
/// 上段: `PROFILE DATA` タイトル (GEAR RATIOS と同サイズ)。
/// 次行: CarID/PI (左) と Status (右寄せ) を同一行に表示。
/// 末尾行: ホットキーヒントを右寄せ表示。
fn draw_status(ui: &Ui, rect: Rect, state: &AppState, s: f32) {
    let painter = ui.painter();
    let saved = state.has_current_profile();

    // -------- タイトル: "PROFILE DATA" (GEAR RATIOS と同じく 3 段拡大) --------
    let title_font_size = 17.5 * 1.520_9 * s;
    let title_font = FontId::new(title_font_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        "PROFILE DATA",
        title_font,
        Color32::from_white_alpha(180),
    );

    // 本文フォント: タイトルより少しだけ小さいサイズ (タイトルを超えない範囲で最大化)
    let body_size = (title_font_size * 0.9).max(6.0 * s);
    let line_gap = 3.0 * s;

    // -------- CarID / PI (左) と Status (右) を同一行に --------
    let row_y = rect.top() + title_font_size + line_gap;
    let body_font = FontId::new(body_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    let car_text = format!(
        "CarID: {}  PI: {}",
        state.last_car_ordinal, state.last_car_pi
    );
    painter.text(
        Pos2::new(rect.left(), row_y),
        Align2::LEFT_TOP,
        car_text,
        body_font.clone(),
        Color32::from_white_alpha(180),
    );

    // Status: "Status : " ラベル + SAVED/RECORDING バッジ を右寄せで配置
    let (badge_text, badge_color) = if saved {
        ("SAVED", Color32::from_rgb(100, 230, 100))
    } else {
        ("RECORDING", Color32::from_rgb(255, 230, 60))
    };
    let label_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            "Status : ".to_string(),
            body_font.clone(),
            Color32::from_white_alpha(180),
        )
    });
    let badge_galley = ui.fonts(|f| {
        f.layout_no_wrap(badge_text.to_string(), body_font.clone(), badge_color)
    });
    let label_w = label_galley.size().x;
    let badge_w = badge_galley.size().x;
    let status_left = rect.right() - (label_w + badge_w);
    painter.galley(
        Pos2::new(status_left, row_y),
        label_galley,
        Color32::from_white_alpha(180),
    );
    painter.galley(
        Pos2::new(status_left + label_w, row_y),
        badge_galley,
        badge_color,
    );

    // -------- ホットキー行 (次行に右寄せ、本文と同じ 2 段拡大サイズ) --------
    let hotkey_y = row_y + body_size + line_gap;
    painter.text(
        Pos2::new(rect.right(), hotkey_y),
        Align2::RIGHT_TOP,
        "Alt+S: Save , Alt+D: Reset",
        body_font,
        Color32::from_white_alpha(120),
    );
}
