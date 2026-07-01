//! 速度表示ウィジェット。
//! 整数部 (3 桁・大フォント) と小数部 (2 桁・小フォント) を横並びにし、
//! 上部に "km/h" または "mph" ラベルを表示する。
//! 速度単位は `AppState::speed_unit_kmh` で切り替え。

use egui::{Align2, Color32, FontFamily, FontId, Rounding, Sense, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

/// scale = 1.0 のウィジェットサイズ (整数 3 桁 + 小数 2 桁が余裕を持って入る幅)
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(160.0, 72.0);

pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let s = scale.x.min(scale.y);
    let size = Vec2::new(INTRINSIC_SIZE.x * scale.x, INTRINSIC_SIZE.y * scale.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();

    // 速度換算
    let speed = if state.speed_unit_kmh {
        state.latest.speed_mps * 3.6
    } else {
        state.latest.speed_mps * 2.236_94
    };
    let unit_label = if state.speed_unit_kmh { "km/h" } else { "mph" };

    let speed = speed.max(0.0);
    let int_part = speed as u32;
    let dec_part = ((speed.fract() * 100.0).round() as u32).min(99);
    let int_text = format!("{int_part:3}");
    let dec_text = format!(".{dec_part:02}");

    let label_size = 11.0 * s; // 少し小さめ
    let int_size = 42.0 * s;
    let dec_size = 24.0 * s;
    let smalltext_pad_y = 3.0 * s;
    let pad = state.input_text_pad * s;

    let int_font = FontId::new(int_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    let dec_font = FontId::new(dec_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    let label_font = FontId::new(label_size, FontFamily::Name(fonts::MONTSERRAT.into()));

    // "000" と ".00" の幅・高さを計測して背景矩形と位置決めに使う
    let (int_w, int_h) = ui.fonts(|f| {
        let sz = f
            .layout_no_wrap("000".to_string(), int_font.clone(), Color32::WHITE)
            .size();
        (sz.x, sz.y)
    });
    let dec_w = ui.fonts(|f| {
        f.layout_no_wrap(".00".to_string(), dec_font.clone(), Color32::WHITE)
            .size()
            .x
    });

    let content_w = int_w + dec_w;
    // 縦は数値のみ (ラベルは数値の上に重ねるので高さに含めない)
    let content_h = int_size;

    let bg_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(content_w + 2.0 * pad, content_h + 2.0 * pad),
    );

    // 角丸半透明背景
    if state.input_text_bg_alpha > 0 {
        painter.rect_filled(
            bg_rect,
            Rounding::same(6.0 * s),
            Color32::from_black_alpha(state.input_text_bg_alpha),
        );
    }

    let cx = bg_rect.center().x;
    // 数値ベースライン: 整数グリフの実測高さ (int_h) を使って視覚的中心を bg_rect 中央に揃える。
    // gear ウィジェットが CENTER_CENTER で bg_rect.center() に配置しているのと同じ基準。
    // (小数・ラベルも nums_y 基準で計算されるため、同じ量だけ Y 方向にシフトされる)
    let nums_y = bg_rect.center().y + int_h / 2.0;

    // 整数部と小数部の X 座標 (中央寄せ)
    // seam_x: 整数部の右端 = 小数部の左端
    let seam_x = cx + (int_w - dec_w) / 2.0;

    // 単位ラベル: 小数部の真上に配置 
    let label_x = seam_x + dec_w / 2.0;
    let label_x_pad = 2.0 * s; // ラベルと小数部の間に少し余白を入れる
    let label_y = nums_y - dec_size - smalltext_pad_y - 2.0 * s;
    painter.text(
        egui::pos2(label_x + label_x_pad, label_y),
        Align2::CENTER_BOTTOM,
        unit_label,
        label_font,
        Color32::from_white_alpha(200),
    );

    // 整数部
    painter.text(
        egui::pos2(seam_x, nums_y),
        Align2::RIGHT_BOTTOM,
        &int_text,
        int_font,
        Color32::WHITE,
    );
    // 小数部 (少し上げる)
    painter.text(
        egui::pos2(seam_x, nums_y - smalltext_pad_y),
        Align2::LEFT_BOTTOM,
        &dec_text,
        dec_font,
        Color32::from_white_alpha(200),
    );
}
