//! スリップインジケーターウィジェット (フロント / リア 2 分割)。
//!
//! `tire_slip_angle` (正規化スリップアングル、|1.0| ≈ グリップ限界) を
//! 前後アクスルごとに 1 行のラベル + 1 行のバーで表示する。
//!
//!   フロント: ラベル "FL SLIP FR" / バー 左=FL, 右=FR
//!   リア  : ラベル "RL SLIP RR" / バー 左=RL, 右=RR
//!
//! 左側 (FL/RL) のバーはウィジェット中央側から外側へ向かって伸びるよう、
//! 右側 (FR/RR) とは逆方向 (右→左) に塗る。
//!
//! 各バーは 0..1.25 のレンジで、0.25/0.5/0.75 に小目盛り (以前の半分の高さ)、
//! 1.00 に長い目盛りを描く。1.00 未満は緑、1.00 以上 1.25 未満は黄色、
//! 1.25 (最大値) 以上は赤で塗る。
//!
//! `AppState::ignore_inward_slip` (既定 ON) が有効なとき、内側方向のスリップは
//! 0 とみなす: L 側 (FL/RL) は正の `tire_slip_angle` のみ、R 側 (FR/RR) は
//! 負の `tire_slip_angle` のみを表示に使う (逆方向の微小な振れをノイズとして無視する)。

use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

/// scale = 1.0 のウィジェットサイズ (ラベル 1 行 + バー 1 行)
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(360.0, 48.0);

/// ゲージ表示レンジ上限
const GAUGE_MAX: f32 = 1.25;
/// 黄色に変わる閾値 (これ以上 GAUGE_MAX 未満)
const WARN_THRESHOLD: f32 = 1.0;

/// 1.00 未満 (グリップに余裕がある) の色
const NORMAL_COLOR: Color32 = Color32::from_rgb(110, 220, 130);
const WARN_COLOR: Color32 = Color32::from_rgb(240, 210, 80);
const OVER_COLOR: Color32 = Color32::from_rgb(255, 80, 60);

/// L/R 側の生スリップ値に方向フィルタを適用する。
/// `ignore_inward` が false なら単純に絶対値を返す。
/// true の場合、L 側 (`is_left` = true) は正の値のみ、R 側は負の値のみを
/// 有効とみなし、逆方向 (内側) は 0 として扱う。
fn directional_slip(raw: f32, is_left: bool, ignore_inward: bool) -> f32 {
    if !ignore_inward {
        return raw.abs();
    }
    if is_left {
        raw.max(0.0)
    } else {
        (-raw).max(0.0)
    }
}

pub fn paint_front(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let angle = &state.latest.tire_slip_angle;
    let ignore_inward = state.ignore_inward_slip;
    let left = directional_slip(angle[0], true, ignore_inward);
    let right = directional_slip(angle[1], false, ignore_inward);
    paint_row(ui, scale, "FL", "FR", left, right);
}

pub fn paint_rear(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let angle = &state.latest.tire_slip_angle;
    let ignore_inward = state.ignore_inward_slip;
    let left = directional_slip(angle[2], true, ignore_inward);
    let right = directional_slip(angle[3], false, ignore_inward);
    paint_row(ui, scale, "RL", "RR", left, right);
}

/// ラベル行 + バー行 (左右 2 本) を描画する共通処理。
fn paint_row(
    ui: &mut Ui,
    scale: Vec2,
    left_label: &str,
    right_label: &str,
    left_value: f32,
    right_value: f32,
) {
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
    let content = Rect::from_min_max(
        Pos2::new(rect.left() + pad, rect.top() + pad),
        Pos2::new(rect.right() - pad, rect.bottom() - pad),
    );

    let label_h = 14.0 * s;
    let gap = 4.0 * s;
    let bar_rect = Rect::from_min_max(
        Pos2::new(content.left(), content.top() + label_h + gap),
        content.max,
    );
    let label_rect = Rect::from_min_max(
        content.min,
        Pos2::new(content.right(), content.top() + label_h),
    );

    let font = FontId::new(12.0 * s, FontFamily::Name(fonts::MONTSERRAT.into()));

    // --- ラベル行: 左輪 .. SLIP .. 右輪 ---
    painter.text(
        Pos2::new(label_rect.left(), label_rect.center().y),
        Align2::LEFT_CENTER,
        left_label,
        font.clone(),
        Color32::from_white_alpha(210),
    );
    painter.text(
        label_rect.center(),
        Align2::CENTER_CENTER,
        "SLIP",
        font.clone(),
        Color32::from_white_alpha(160),
    );
    painter.text(
        Pos2::new(label_rect.right(), label_rect.center().y),
        Align2::RIGHT_CENTER,
        right_label,
        font,
        Color32::from_white_alpha(210),
    );

    // --- バー行 (左右 2 本) ---
    let gap_x = 6.0 * s;
    let half_w = (bar_rect.width() - gap_x) * 0.5;
    let left_rect = Rect::from_min_max(
        bar_rect.min,
        Pos2::new(bar_rect.left() + half_w, bar_rect.bottom()),
    );
    let right_rect = Rect::from_min_max(
        Pos2::new(bar_rect.right() - half_w, bar_rect.top()),
        bar_rect.max,
    );

    // 左側 (FL/RL) は中央側から外側 (右→左) へ、右側 (FR/RR) は通常 (左→右) に塗る。
    draw_slip_bar(painter, left_rect, left_value, true, s);
    draw_slip_bar(painter, right_rect, right_value, false, s);
}

/// 1 輪ぶんの水平ゲージ。0..GAUGE_MAX、0.25 刻みの小目盛り、1.00 に長い目盛り。
/// 1.00 未満で緑、1.00 以上 GAUGE_MAX 未満で黄、GAUGE_MAX (1.25) 以上で赤。
/// `reversed` = true のとき、右端を起点に左へ向かって塗る (左輪用)。
fn draw_slip_bar(painter: &egui::Painter, rect: Rect, value: f32, reversed: bool, s: f32) {
    // バー背景
    painter.rect_filled(
        rect,
        Rounding::same(2.0 * s),
        Color32::from_rgba_unmultiplied(0, 0, 0, 160),
    );

    // 塗り
    let t = (value / GAUGE_MAX).clamp(0.0, 1.0);
    if t > 0.0 {
        let color = if value >= GAUGE_MAX {
            OVER_COLOR
        } else if value >= WARN_THRESHOLD {
            WARN_COLOR
        } else {
            NORMAL_COLOR
        };
        let fill = if reversed {
            Rect::from_min_max(
                Pos2::new(rect.right() - rect.width() * t, rect.top() + 1.0 * s),
                Pos2::new(rect.right() - 1.0 * s, rect.bottom() - 1.0 * s),
            )
        } else {
            Rect::from_min_max(
                Pos2::new(rect.left() + 1.0 * s, rect.top() + 1.0 * s),
                Pos2::new(rect.left() + rect.width() * t, rect.bottom() - 1.0 * s),
            )
        };
        painter.rect_filled(fill, Rounding::ZERO, color);
    }

    // 0.25 刻みの小目盛り (高さは以前の半分) + 1.00 の長い目盛り
    let mut mark = 0.25_f32;
    while mark < GAUGE_MAX {
        let frac = mark / GAUGE_MAX;
        let x = if reversed {
            rect.right() - rect.width() * frac
        } else {
            rect.left() + rect.width() * frac
        };
        let long = (mark - 1.0).abs() < 0.001; // 1.00 は長い目盛り
        let (y0, stroke) = if long {
            (
                rect.top(),
                Stroke::new(1.5 * s, Color32::from_white_alpha(220)),
            )
        } else {
            // 小目盛りの高さ: 全体の 35% (以前の 70% の半分)
            (
                rect.top() + rect.height() * 0.65,
                Stroke::new(1.0 * s, Color32::from_white_alpha(90)),
            )
        };
        painter.line_segment([Pos2::new(x, y0), Pos2::new(x, rect.bottom())], stroke);
        mark += 0.25;
    }

    // 枠
    painter.rect_stroke(
        rect,
        Rounding::same(2.0 * s),
        Stroke::new(1.0 * s, Color32::from_white_alpha(60)),
    );
}
