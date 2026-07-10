//! テレメトリデバッグウィジェット。
//! 受信したパケットの全フィールドを「項目名: 値」形式で縦に並べて表示する。
//! デフォルト非表示 (Settings で visible = true にすると現れる)。

use egui::{Color32, FontFamily, FontId, Ui, Vec2};

use crate::state::AppState;

/// scale = 1.0 のウィジェットサイズ。
/// 行数 × 行高で縦方向を決める。
pub const INTRINSIC_SIZE: Vec2 = Vec2::new(340.0, 520.0);

/// 1 行の高さ (scale = 1.0)
const LINE_H: f32 = 14.0;

pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
    let s = scale.y.max(0.1);
    let font_size = LINE_H * s;
    let font = FontId::new(font_size, FontFamily::Monospace);

    let t = &state.latest;

    // 表示する行を (ラベル, 値文字列) のスライスとして定義
    let gear_str = if t.gear == 0 {
        "R".to_string()
    } else {
        t.gear.to_string()
    };

    let rows: &[(&str, String)] = &[
        ("is_race_on", format!("{}", t.is_race_on)),
        ("timestamp_ms", format!("{}", t.timestamp_ms)),
        // --- Engine ---
        ("engine_max_rpm", format!("{:.1}", t.engine_max_rpm)),
        ("engine_idle_rpm", format!("{:.1}", t.engine_idle_rpm)),
        ("current_rpm", format!("{:.1}", t.current_rpm)),
        // --- Speed / Power ---
        ("speed_mps", format!("{:.2}", t.speed_mps)),
        ("speed_kph", format!("{:.1}", t.speed_kph())),
        ("power_w", format!("{:.1}", t.power_w)),
        ("power_hp", format!("{:.1}", t.power_hp())),
        ("torque_nm", format!("{:.1}", t.torque_nm)),
        // --- Inputs ---
        ("accel", format!("{:.4}", t.accel)),
        ("brake", format!("{:.4}", t.brake)),
        ("clutch", format!("{:.4}", t.clutch)),
        ("handbrake", format!("{:.4}", t.handbrake)),
        ("steer", format!("{:.4}", t.steer)),
        (
            "NormalizedDrivingLine",
            format!(
                "{} ({:+.4})",
                t.normalized_driving_line,
                t.normalized_driving_line as f32 / 127.0
            ),
        ),
        (
            "NormalizedAIBrakeDifference",
            format!(
                "{} ({:+.4})",
                t.normalized_ai_brake_difference,
                t.normalized_ai_brake_difference as f32 / 127.0
            ),
        ),
        ("gear", gear_str),
        // --- Vehicle ID ---
        ("car_ordinal", format!("{}", t.car_ordinal)),
        ("car_perf_index", format!("{}", t.car_performance_index)),
        // --- Tire slip ---
        ("tire_slip[FL]", format!("{:.4}", t.tire_slip[0])),
        ("tire_slip[FR]", format!("{:.4}", t.tire_slip[1])),
        ("tire_slip[RL]", format!("{:.4}", t.tire_slip[2])),
        ("tire_slip[RR]", format!("{:.4}", t.tire_slip[3])),
        ("max_tire_slip", format!("{:.4}", t.max_tire_slip())),
        // --- Tire slip angle ---
        ("slip_angle[FL]", format!("{:+.4}", t.tire_slip_angle[0])),
        ("slip_angle[FR]", format!("{:+.4}", t.tire_slip_angle[1])),
        ("slip_angle[RL]", format!("{:+.4}", t.tire_slip_angle[2])),
        ("slip_angle[RR]", format!("{:+.4}", t.tire_slip_angle[3])),
        ("g_bar_max_g", format!("{:.1}", state.g_bar_max_g)),
        // --- App-computed ---
        (
            "last_engine_max_rpm",
            format!("{:.1}", state.last_engine_max_rpm),
        ),
        (
            "rev_limit",
            state.rev_limit.map_or("--".into(), |v| format!("{:.1}", v)),
        ),
        // --- Acceleration (m/s²) ---
        ("accel_x [right+]", format!("{:+.3}", t.accel_x)),
        ("accel_y [up+]", format!("{:+.3}", t.accel_y)),
        ("accel_z [fwd+]", format!("{:+.3}", t.accel_z)),
        // --- G-force (raw) ---
        ("lateral_g  [R+]", format!("{:+.3}", t.lateral_g())),
        ("longitud_g [F+]", format!("{:+.3}", t.longitudinal_g())),
        ("vertical_g [U+]", format!("{:+.3}", t.vertical_g())),
        ("total_g", format!("{:.3}", t.total_g())),
        // --- G-force (EMA smoothed components) ---
        ("ema_accel_x", format!("{:+.3}", state.accel_ema[0])),
        ("ema_accel_y", format!("{:+.3}", state.accel_ema[1])),
        ("ema_accel_z", format!("{:+.3}", state.accel_ema[2])),
        (
            "sm_lateral_g  [R+]",
            format!("{:+.3}", state.smoothed_lateral_g()),
        ),
        (
            "sm_longitud_g [F+]",
            format!("{:+.3}", state.smoothed_longitudinal_g()),
        ),
        (
            "sm_vertical_g [U+]",
            format!("{:+.3}", state.smoothed_vertical_g()),
        ),
        ("sm_total_g", format!("{:.3}", state.smoothed_total_g())),
    ];

    // ラベル列の最大文字数を固定幅フォントで揃えるため、最長ラベルを求める
    let label_col = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);

    let painter = ui.painter();
    let origin = ui.cursor().min;

    for (i, (key, val)) in rows.iter().enumerate() {
        let y = origin.y + i as f32 * font_size * 1.2;
        let pos = egui::pos2(origin.x, y);

        // キー (右揃え相当に空白パディング)
        let padded_key = format!("{:>width$}", key, width = label_col);
        let line = format!("{}: {}", padded_key, val);

        painter.text(
            pos,
            egui::Align2::LEFT_TOP,
            &line,
            font.clone(),
            Color32::from_white_alpha(220),
        );
    }

    // レイアウト領域を確保 (allocate しないと次のウィジェットが重なる)
    let total_h = rows.len() as f32 * font_size * 1.2;
    ui.allocate_space(Vec2::new(INTRINSIC_SIZE.x * scale.x, total_h));
}
