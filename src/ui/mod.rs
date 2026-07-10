//! オーバーレイ用ウィジェット群。
//!
//! 新しいウィジェットを追加する手順:
//! 1. `src/ui/<name>.rs` を作る (`pub const INTRINSIC_SIZE: Vec2`, `pub fn paint(ui, state, scale)` を実装)
//! 2. `src/state.rs` の `Layout` に `pub <name>: LayoutItem` フィールド + `default_<name>` を追加
//! 3. このファイル末尾の `WIDGETS` 配列に `WidgetSpec { ... }` を追加
//! 4. `main.rs` 側は変更不要 (`WIDGETS` を順に子ビューポート化する)

use egui::{Ui, Vec2};

use crate::state::{AppState, Layout, LayoutItem};

pub mod editor;
pub mod fonts;
pub mod g_bar;
pub mod gear;
pub mod input_text;
pub mod shift;
pub mod slip_indicator;
pub mod speed;
pub mod stats;
pub mod telemetry_debug;

/// 子ビューポートの外枠 (`Frame`) スタイル。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetFrameStyle {
    /// 半透明黒の背景 + 角丸 + 白っぽい細枠 (デフォルト)
    Boxed,
    /// 完全に透明 (背景・枠なし)。ウィジェット側で必要なら自前描画する
    Transparent,
}

/// 1 つのオーバーレイウィジェットを記述するエントリ。
pub struct WidgetSpec {
    /// 内部 ID (ViewportId 生成と HWND 検索タイトルに利用)
    pub id: &'static str,
    /// 設定ウィンドウに表示する人間向けラベル
    pub label: &'static str,
    /// scale = 1.0 のときのウィジェットサイズ
    pub intrinsic: Vec2,
    /// 配置情報の取得 (可変)
    pub get_mut: fn(&mut Layout) -> &mut LayoutItem,
    /// 配置情報の取得 (不変)
    pub get: fn(&Layout) -> &LayoutItem,
    /// ウィジェット本体を描画する関数 (scale は X/Y 独立倍率)
    pub paint: fn(&mut Ui, &AppState, Vec2),
    /// 子ビューポートの背景スタイル
    pub frame_style: WidgetFrameStyle,
}

pub const WIDGETS: &[WidgetSpec] = &[
    WidgetSpec {
        id: "stats",
        label: "Stats",
        intrinsic: stats::INTRINSIC_SIZE,
        get_mut: |l| &mut l.stats,
        get: |l| &l.stats,
        paint: stats::paint,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "shift",
        label: "Shift indicator",
        intrinsic: shift::INTRINSIC_SIZE,
        get_mut: |l| &mut l.shift,
        get: |l| &l.shift,
        paint: shift::paint,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "gear_display",
        label: "Gear",
        intrinsic: gear::INTRINSIC_SIZE,
        get_mut: |l| &mut l.gear_display,
        get: |l| &l.gear_display,
        paint: gear::paint,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "speed_display",
        label: "Speed",
        intrinsic: speed::INTRINSIC_SIZE,
        get_mut: |l| &mut l.speed_display,
        get: |l| &l.speed_display,
        paint: speed::paint,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "acc_text",
        label: "ACC (text)",
        intrinsic: input_text::INTRINSIC_SIZE,
        get_mut: |l| &mut l.acc_text,
        get: |l| &l.acc_text,
        paint: input_text::paint_acc,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "brk_text",
        label: "BRK (text)",
        intrinsic: input_text::INTRINSIC_SIZE,
        get_mut: |l| &mut l.brk_text,
        get: |l| &l.brk_text,
        paint: input_text::paint_brk,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "g_bar",
        label: "Lateral G bar",
        intrinsic: g_bar::INTRINSIC_SIZE,
        get_mut: |l| &mut l.g_bar,
        get: |l| &l.g_bar,
        paint: g_bar::paint,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "slip_front",
        label: "Front slip indicator",
        intrinsic: slip_indicator::INTRINSIC_SIZE,
        get_mut: |l| &mut l.slip_front,
        get: |l| &l.slip_front,
        paint: slip_indicator::paint_front,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "slip_rear",
        label: "Rear slip indicator",
        intrinsic: slip_indicator::INTRINSIC_SIZE,
        get_mut: |l| &mut l.slip_rear,
        get: |l| &l.slip_rear,
        paint: slip_indicator::paint_rear,
        frame_style: WidgetFrameStyle::Transparent,
    },
    WidgetSpec {
        id: "telemetry_debug",
        label: "Telemetry Debug (all fields)",
        intrinsic: telemetry_debug::INTRINSIC_SIZE,
        get_mut: |l| &mut l.telemetry_debug,
        get: |l| &l.telemetry_debug,
        paint: telemetry_debug::paint,
        frame_style: WidgetFrameStyle::Boxed,
    },
];
