//! 横 (左右) G バーウィジェット。
//!
//! 中央がニュートラル (0G)、実際に G がかかっている方向へバーが伸びる。
//! 右方向 (`accel_x` 正) = 右 G、左方向 = 左 G。
//! 表示レンジ (最大 G) は `AppState::g_bar_max_g` (既定 4.0G) で、設定パネルの
//! ウィジェット調整画面から変更できる。レンジを超えたら赤で振り切れを示す。

use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

/// scale = 1.0 のウィジェットサイズ (高さは半分に縮小済み)
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(360.0, 25.0);

const BAR_COLOR: Color32 = Color32::from_rgb(120, 200, 255);
const OVER_COLOR: Color32 = Color32::from_rgb(255, 80, 60);

pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let size = Vec2::new(INTRINSIC_SIZE.x * scale.x, INTRINSIC_SIZE.y * scale.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    let s = scale.x.min(scale.y).max(0.1);

    // 丸角黒半透明背景 (他のウィジェットと同様)
    painter.rect_filled(
        rect,
        Rounding::same(6.0 * s),
        Color32::from_rgba_unmultiplied(0, 0, 0, 150),
    );

    let pad = 6.0 * s;
    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + pad, rect.top() + pad),
        Pos2::new(rect.right() - pad, rect.bottom() - pad),
    );

    let max_g = state.g_bar_max_g.max(0.1);
    let cx = bar.center().x;
    let half_w = bar.width() * 0.5;

    // 1G 刻みの目盛り (中央からの整数 G 位置に縦線)
    let mut g = 1.0_f32;
    while g < max_g {
        let dx = half_w * (g / max_g);
        for x in [cx - dx, cx + dx] {
            painter.line_segment(
                [Pos2::new(x, bar.top()), Pos2::new(x, bar.bottom())],
                Stroke::new(1.0 * s, Color32::from_white_alpha(30)),
            );
        }
        g += 1.0;
    }

    let lat_g = state.smoothed_lateral_g();
    let t = (lat_g / max_g).clamp(-1.0, 1.0);
    let over = lat_g.abs() >= max_g;
    if t.abs() > f32::EPSILON {
        // ゲージ方向を反転: G がかかっている方向と逆側へ伸ばす
        let x_end = cx - t * half_w;
        let color = if over { OVER_COLOR } else { BAR_COLOR };
        let fill = Rect::from_min_max(
            Pos2::new(cx.min(x_end), bar.top() + 1.0 * s),
            Pos2::new(cx.max(x_end), bar.bottom() - 1.0 * s),
        );
        painter.rect_filled(fill, Rounding::ZERO, color);
    }

    // 中央線 (ニュートラル基準)
    painter.line_segment(
        [Pos2::new(cx, bar.top()), Pos2::new(cx, bar.bottom())],
        Stroke::new(2.0 * s, Color32::from_white_alpha(150)),
    );

    // 現在値テキスト (中央に重ねる、視認性のため簡易シャドウ付き)
    let font = FontId::new(14.0 * s, FontFamily::Name(fonts::MONTSERRAT.into()));
    let label = format!("{:+.2} G", lat_g);
    painter.text(
        bar.center() + Vec2::new(1.0 * s, 1.0 * s),
        Align2::CENTER_CENTER,
        &label,
        font.clone(),
        Color32::from_black_alpha(200),
    );
    painter.text(
        bar.center(),
        Align2::CENTER_CENTER,
        &label,
        font,
        Color32::from_white_alpha(230),
    );
}
