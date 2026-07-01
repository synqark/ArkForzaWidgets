//! ギア表示ウィジェット。
//! 現在のギアを大きく表示する。R (リバース) ～ 12 速に対応。
//! スタイルは ACC/BRK テキストウィジェットに準拠。

use egui::{Align2, Color32, FontFamily, FontId, Rounding, Sense, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

/// scale = 1.0 のウィジェットサイズ
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(80.0, 72.0);

pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let s = scale.x.min(scale.y);
    let size = Vec2::new(INTRINSIC_SIZE.x * scale.x, INTRINSIC_SIZE.y * scale.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();

    let num_size = 42.0 * s;
    let pad = state.input_text_pad * s;

    // "12" が最大幅なので、それを基準に背景矩形の幅を決める
    let num_font_id = FontId::new(num_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    let max_w = ui.fonts(|f| {
        f.layout_no_wrap("12".to_string(), num_font_id.clone(), Color32::WHITE)
            .size()
            .x
    });

    // ラベルなし: 数値のみの高さで矩形を作る
    let bg_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(max_w + 2.0 * pad, num_size + 2.0 * pad),
    );

    // 角丸半透明背景
    if state.input_text_bg_alpha > 0 {
        painter.rect_filled(
            bg_rect,
            Rounding::same(6.0 * s),
            Color32::from_black_alpha(state.input_text_bg_alpha),
        );
    }

    let gear = state.latest.gear;

    // ギアテキストと色
    //   0  → "R" (オレンジ)
    //   11 → "N" (少し暗め: ニュートラル)
    //   1..=12 → 数字 (白)
    let (gear_text, color) = match gear {
        0 => ("R".to_string(), Color32::from_rgb(255, 120, 40)),
        11 => ("N".to_string(), Color32::from_white_alpha(140)),
        g => (format!("{g}"), Color32::WHITE),
    };

    painter.text(
        egui::pos2(bg_rect.center().x, bg_rect.center().y),
        Align2::CENTER_CENTER,
        gear_text,
        num_font_id,
        color,
    );
}
