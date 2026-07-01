//! カスタムフォント (Montserrat Italic) の登録。
//!
//! 起動時に 1 度だけ `install(&cc.egui_ctx)` を呼ぶ。
//! フォントファイルは exe に `include_bytes!` で埋め込まれている。
//! ライセンス: SIL Open Font License 1.1 (OFL) — 商用利用・同梱・再配布すべて可。

use egui::{Context, FontData, FontDefinitions, FontFamily};

/// `FontFamily::Name(MONTSERRAT.into())` で参照する。
pub const MONTSERRAT: &str = "montserrat";

static MONTSERRAT_ITALIC: &[u8] = include_bytes!("../../assets/Montserrat-Italic.ttf");

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        MONTSERRAT.to_string(),
        FontData::from_static(MONTSERRAT_ITALIC),
    );
    fonts.families.insert(
        FontFamily::Name(MONTSERRAT.into()),
        vec![MONTSERRAT.to_string()],
    );
    log::info!("loaded font: Montserrat Italic (embedded)");
    ctx.set_fonts(fonts);
}
