//! シフトインジケーターウィジェット。
//!
//! 保存済みプロファイル (rev_limit あり) が前提の横長プログレスバー。
//!   - 表示レンジ: 左端 = シフトダウン位置 -10%、右端 = シフトアップ位置 +10%
//!     (= 「アップ - ダウン」+ 20% 相当)
//!   - 現在値を左端からの白い塗りで表示 (バーを上下 2 分割)
//!     - 上半分 = 実測速度ベース (現ギア比で rpm 相当に換算)
//!     - 下半分 = 現在 RPM ベース
//!     ホイールスピン/滑り中は下だけ先に伸びる
//!   - シフトアップ / シフトダウン位置を上下の三角形 (▼▲) で表示
//!     - アップ = 現ギアの最適シフト RPM (車輪推進力の交点)
//!     - ダウン = 下ギアへの最適シフトダウン RPM (1 速など無ければ非表示)
//!   - rev_limit を超える領域は半透明赤で塗る
//!
//! 未保存の場合はダイノ記録を促す 1 行のテキストのみ表示する。

use egui::{Align2, Color32, FontFamily, FontId, Rect, Rounding, Sense, Stroke, Ui, Vec2};

use crate::state::AppState;
use crate::ui::fonts;

/// scale = 1.0 のウィジェットサイズ (三角形の上下余白込み)
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(420.0, 80.0);

pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let size = Vec2::new(INTRINSIC_SIZE.x * scale.x, INTRINSIC_SIZE.y * scale.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    let s = scale.x.min(scale.y);

    // 丸角黒半透明背景 (他のウィジェットと同様)
    painter.rect_filled(
        rect,
        Rounding::same(6.0 * s),
        Color32::from_rgba_unmultiplied(0, 0, 0, 150),
    );

    // 保存済みプロファイル + rev_limit が無ければ案内テキストのみ
    let profile = state.current_profile();
    let rev_limit = profile.and_then(|p| p.rev_limit);
    let (profile, rev_limit) = match (profile, rev_limit) {
        (Some(p), Some(r)) if r > 0.0 => (p, r),
        _ => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Record & save a dyno profile to enable the shift indicator",
                FontId::new(13.0 * s, FontFamily::Name(fonts::MONTSERRAT.into())),
                Color32::from_white_alpha(180),
            );
            return;
        }
    };

    // シフトアップ位置:
    //   1. ギア比が記録済みなら「現ギアの最適シフト RPM」(車輪推進力の交点)
    //   2. 無ければ従来どおり min(パワーバンド終端, rev_limit)
    let gear = state.latest.gear;
    let shift_up = profile
        .optimal_shift_rpm(gear, rev_limit)
        .unwrap_or_else(|| match profile.power_band() {
            Some((_, be)) => be.min(rev_limit),
            None => rev_limit,
        });
    // シフトダウン位置 (現ギア → 下ギア)。1 速やギア比未記録だと None。
    let shift_down = profile.optimal_downshift_rpm(gear, rev_limit);

    // バー表示レンジ:
    //   左端 = シフトダウン位置 - 10% (無ければアップ位置の 70%)
    //   右端 = シフトアップ位置 + 10% (rev_limit でクランプしない)
    let lo = match shift_down {
        Some(d) => (d * 0.90).max(1.0),
        None => (shift_up * 0.70).max(1.0),
    };
    let hi = (shift_up * 1.10).max(lo + 1.0);
    let span = (hi - lo).max(1.0);

    // レイアウト定数
    let pad = 6.0 * s;       // 外側の余白
    let tri_h = 10.0 * s;    // 三角形の高さ
    let label_w = 36.0 * s;  // 「SPD」「RPM」ラベルの幅
    let gap = 5.0 * s;       // SPDバーとRPMバーの隙間
    let bar_h = (size.y - pad * 2.0 - tri_h * 2.0 - gap) / 2.0;

    // SPDバーとRPMバーの矩形 (左右は三角形用スペースを除き、label_w 分を外側ラベル領域に)
    let bar_left = rect.left() + label_w + 4.0 * s;
    let bar_right = rect.right() - label_w - 4.0 * s;
    let spd_bar = Rect::from_min_max(
        egui::pos2(bar_left, rect.top() + pad + tri_h),
        egui::pos2(bar_right, rect.top() + pad + tri_h + bar_h),
    );
    let rpm_bar = Rect::from_min_max(
        egui::pos2(bar_left, spd_bar.bottom() + gap),
        egui::pos2(bar_right, spd_bar.bottom() + gap + bar_h),
    );

    // rpm -> バー内 X 座標 (clamp)
    let to_x = |rpm: f32| -> f32 {
        let t = ((rpm - lo) / span).clamp(0.0, 1.0);
        bar_left + t * (bar_right - bar_left)
    };

    let k_cur = profile.gear_ratio(gear); // rpm / kph
    let speed_kph = state.latest.speed_kph();
    let rpm = state.latest.current_rpm;

    // --- SPDバー ---
    // 背景
    painter.rect_filled(
        spd_bar,
        Rounding::ZERO,
        Color32::from_rgba_unmultiplied(0, 0, 0, 160),
    );
    // 速度ベース塗り (k_cur があるときのみ。無ければ rpm で代用)
    let speed_equiv_rpm = match k_cur {
        Some(k) if k > 0.0 => speed_kph * k,
        _ => rpm,
    };
    if speed_equiv_rpm > 0.0 {
        let x = to_x(speed_equiv_rpm);
        if x > spd_bar.left() {
            painter.rect_filled(
                Rect::from_min_max(spd_bar.min, egui::pos2(x, spd_bar.bottom())),
                Rounding::ZERO,
                Color32::from_white_alpha(200),
            );
        }
    }
    // 白い枠
    painter.rect_stroke(
        spd_bar,
        Rounding::ZERO,
        Stroke::new((1.5 * s).max(1.0), Color32::from_white_alpha(180)),
    );
    // 「SPD」ラベル (両端)
    let label_font = FontId::new(12.0 * s, FontFamily::Name(fonts::MONTSERRAT.into()));
    let label_color = Color32::from_white_alpha(200);
    painter.text(
        egui::pos2(spd_bar.left() - 4.0 * s, spd_bar.center().y),
        Align2::RIGHT_CENTER,
        "SPD",
        label_font.clone(),
        label_color,
    );
    painter.text(
        egui::pos2(spd_bar.right() + 4.0 * s, spd_bar.center().y),
        Align2::LEFT_CENTER,
        "SPD",
        label_font.clone(),
        label_color,
    );

    // --- RPMバー ---
    // 背景
    painter.rect_filled(
        rpm_bar,
        Rounding::ZERO,
        Color32::from_rgba_unmultiplied(0, 0, 0, 160),
    );
    // 現在 RPM ベース塗り
    if rpm > 0.0 {
        let x = to_x(rpm);
        if x > rpm_bar.left() {
            painter.rect_filled(
                Rect::from_min_max(rpm_bar.min, egui::pos2(x, rpm_bar.bottom())),
                Rounding::ZERO,
                Color32::from_white_alpha(200),
            );
        }
    }
    // rev_limit を超える領域を半透明赤で塗る (RPMバーのみ)
    if rev_limit < hi {
        let xr = to_x(rev_limit);
        if rpm_bar.right() > xr {
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(xr, rpm_bar.top()),
                    egui::pos2(rpm_bar.right(), rpm_bar.bottom()),
                ),
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(220, 40, 40, 140),
            );
        }
    }
    // 白い枠
    painter.rect_stroke(
        rpm_bar,
        Rounding::ZERO,
        Stroke::new((1.5 * s).max(1.0), Color32::from_white_alpha(180)),
    );
    // 「RPM」ラベル (両端)
    painter.text(
        egui::pos2(rpm_bar.left() - 4.0 * s, rpm_bar.center().y),
        Align2::RIGHT_CENTER,
        "RPM",
        label_font.clone(),
        label_color,
    );
    painter.text(
        egui::pos2(rpm_bar.right() + 4.0 * s, rpm_bar.center().y),
        Align2::LEFT_CENTER,
        "RPM",
        label_font.clone(),
        label_color,
    );

    // 上下の三角形 (▼▲) をある rpm 位置に描くヘルパ
    // ▼ は SPDバー上、▲ は RPMバー下を指す
    let tw = 7.0 * s;
    let draw_triangles = |x: f32| {
        // ▼ SPDバー上 (下向き三角、頂点がSPDバー上端を指す)
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(x - tw, spd_bar.top() - tri_h),
                egui::pos2(x + tw, spd_bar.top() - tri_h),
                egui::pos2(x, spd_bar.top() - 1.0 * s),
            ],
            Color32::WHITE,
            Stroke::NONE,
        ));
        // ▲ RPMバー下 (上向き三角、頂点がRPMバー下端を指す)
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(x - tw, rpm_bar.bottom() + tri_h),
                egui::pos2(x + tw, rpm_bar.bottom() + tri_h),
                egui::pos2(x, rpm_bar.bottom() + 1.0 * s),
            ],
            Color32::WHITE,
            Stroke::NONE,
        ));
    };

    // シフトアップ位置の三角形 + 太い白縦線 (両バーを貫く)
    let sx = to_x(shift_up);
    draw_triangles(sx);
    painter.line_segment(
        [egui::pos2(sx, spd_bar.top()), egui::pos2(sx, spd_bar.bottom())],
        Stroke::new((3.0 * s).max(2.0), Color32::WHITE),
    );
    painter.line_segment(
        [egui::pos2(sx, rpm_bar.top()), egui::pos2(sx, rpm_bar.bottom())],
        Stroke::new((3.0 * s).max(2.0), Color32::WHITE),
    );

    // シフトダウン位置の三角形 (あれば)
    if let Some(d) = shift_down {
        draw_triangles(to_x(d));
    }
}
