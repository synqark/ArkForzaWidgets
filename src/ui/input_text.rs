//! ACC / BRK テキスト表示ウィジェット。
//! 小さいラベルと大きい数値を縦に並べて中央揃えで表示する。
//! 値が 0 または 100 のときは白、それ以外は黄色。

use egui::{Align2, Color32, FontFamily, FontId, Rounding, Sense, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

pub const INTRINSIC_SIZE: Vec2 = Vec2::new(80.0, 72.0);

/// 共通描画ロジック。`label` はヘッダ文字列、`value` は 0.0..=1.0。
fn paint_value(ui: &mut Ui, label: &str, value: f32, scale: Vec2, bg_alpha: u8, extra_pad: f32) {
    let s = scale.x.min(scale.y);
    let size = Vec2::new(INTRINSIC_SIZE.x * scale.x, INTRINSIC_SIZE.y * scale.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();

    let label_size = 14.0 * s;
    let num_size = 42.0 * s;
    let gap = 4.0 * s;

    // "100" の描画幅を実測し、コンテンツサイズを基準に外側へ extra_pad 分拡張して bg_rect を作る。
    // (rect.shrink は pad が増えるほど縮むため、ここでは center + size で構築する)
    let num_font_id = FontId::new(num_size, FontFamily::Name(fonts::MONTSERRAT.into()));
    let w100 = ui.fonts(|f| {
        f.layout_no_wrap("100".to_string(), num_font_id.clone(), Color32::WHITE)
            .size()
            .x
    });
    let pad = extra_pad * s;
    let content_height = label_size + gap + num_size;
    let bg_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(w100 + 2.0 * pad, content_height + 2.0 * pad),
    );

    // 角丸半透明背景
    if bg_alpha > 0 {
        painter.rect_filled(
            bg_rect,
            Rounding::same(6.0 * s),
            Color32::from_black_alpha(bg_alpha),
        );
    }

    let percent = (value * 100.0).round() as i32;
    let is_yellow = percent > 0 && percent < 100;
    // 0 のとき: テキストを半透明化
    // 黄色のとき: 50ms 間隔で表示/非表示を切り替えて点滅
    let color = if percent <= 0 {
        Color32::from_white_alpha(32)
    } else if !is_yellow {
        Color32::WHITE
    } else {
        let t = ui.ctx().input(|i| i.time);
        // 100ms 周期の前半 50ms を点灯、後半 50ms を消灯
        if (t * 20.0) as u64 % 2 == 0 {
            Color32::from_rgb(255, 255, 60)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 32, 128)
        }
    };
    let label_color = Color32::from_white_alpha(255);

    // テキストを bg_rect の中に縦センタリングして配置
    let content_top = bg_rect.center().y - content_height / 2.0;
    let cx = bg_rect.center().x;
    let label_y = content_top + label_size; // ラベル下端 Y
    let num_y = label_y + gap + num_size * 0.5; // 数値中央 Y

    painter.text(
        egui::pos2(cx, label_y),
        Align2::CENTER_BOTTOM,
        label,
        FontId::new(label_size, FontFamily::Name(fonts::MONTSERRAT.into())),
        label_color,
    );
    painter.text(
        egui::pos2(cx, num_y),
        Align2::CENTER_CENTER,
        format!("{}", percent),
        num_font_id,
        color,
    );
}

pub fn paint_acc(ui: &mut Ui, state: &AppState, scale: Vec2) {
    paint_value(
        ui,
        "ACC",
        state.latest.accel,
        scale,
        state.input_text_bg_alpha,
        state.input_text_pad,
    );
}

pub fn paint_brk(ui: &mut Ui, state: &AppState, scale: Vec2) {
    paint_value(
        ui,
        "BRK",
        state.latest.brake,
        scale,
        state.input_text_bg_alpha,
        state.input_text_pad,
    );
}
