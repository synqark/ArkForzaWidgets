# Copilot Instructions — ArkForzaWidgets

このファイルは GitHub Copilot (エージェント) 向けのプロジェクトコンテキスト・開発方針書です。
新しいセッションを開始したら、まずこのファイルを読んでから作業してください。以後の実装判断は
基本的にエージェントに委ねますが、ここに書かれた設計制約・約束事は必ず尊重してください。

---

## プロジェクト概要

**ArkForzaWidgets** (旧名 FHTHD = Forza Horizon Telemetry HUD)

Forza Horizon 6 が UDP で出力するテレメトリ (Data Out) を受信し、ゲーム画面上に半透明オーバーレイで
補助情報を表示する Windows 向けデスクトップアプリ。

### 現在のオーバーレイウィジェット (`src/ui/mod.rs::WIDGETS`)

| id | ラベル | モジュール | 内容 |
|---|---|---|---|
| `stats` | Stats | `stats.rs` | 加速度 G など統計値 |
| `shift` | Shift indicator | `shift.rs` | シフトインジケータ |
| `gear_display` | Gear | `gear.rs` | ギア数値の大型表示 |
| `speed_display` | Speed | `speed.rs` | 速度 (km/h または mph) |
| `acc_text` | ACC (text) | `input_text.rs` (`paint_acc`) | アクセル入力 0..=100 テキスト |
| `brk_text` | BRK (text) | `input_text.rs` (`paint_brk`) | ブレーキ入力 0..=100 テキスト |
| `telemetry_debug` | Telemetry Debug | `telemetry_debug.rs` | 全フィールドのデバッグ表示 (既定非表示) |

- **ダイノグラフ**は現在オーバーレイではなく **設定パネル (`ui/editor.rs`) 内**に `egui_plot` で描画される。
  RPM ごとの Power/Torque ピークホールドとパワーバンド帯ハイライトを表示する。

### 将来追加したい表示 (順不同・未確定)

- タイヤ温度 / スリップ角 / サスペンションストローク
- ラップタイム比較 (ベストとの差分)
- 走行ログの CSV / Parquet 出力

---

## 要件 / 設計制約

| 項目 | 内容 |
|---|---|
| OS | Windows 10/11 (x64) のみ |
| ゲーム | Forza Horizon 6 のみ (FM/FH5 等は対象外) |
| 動作 PC | ゲーム PC と同一 PC を前提 (`127.0.0.1` 受信) |
| 配布形態 | 単体 `ArkForzaWidgets.exe` (+ 任意で `config.toml`) |
| 描画 | NoVsync + UDP イベント駆動 (受信時に present)。低遅延優先 |
| ウィンドウ構成 | **メイン = 設定ウィンドウ (装飾あり)** / **オーバーレイ = 単一の透明子ビューポート (装飾なし・最前面・クリックスルー)** |
| クリックスルー | Win32 EX_STYLE で実装済 (`WS_EX_LAYERED \| WS_EX_TRANSPARENT \| WS_EX_NOACTIVATE \| WS_EX_TOOLWINDOW`) |
| DPI | プロセスを **System DPI Aware** に固定 (後述) |
| ライセンス | zlib License (`LICENSE.md`) |
| 言語 | ユーザーは Rust 未経験。奇抜な書き方より素直で読みやすいコードを優先 |

---

## 技術スタック

| 層 | 採用 | 備考 |
|---|---|---|
| 言語 | Rust 1.96.0 (stable) | rustup 管理 |
| ウィンドウ/イベント | `winit` (eframe 経由) | |
| 描画 | `wgpu` (DX12/Vulkan) | `eframe` の `wgpu` feature |
| UI | `egui` 0.29 (即時モード) | `eframe` 経由 |
| グラフ | `egui_plot` 0.29 | ダイノグラフ (設定パネル内) で使用 |
| UDP | `std::net::UdpSocket` + 専用スレッド | ブロッキング受信が最低遅延 |
| スレッド連携 | `crossbeam-channel` (容量 1, `try_send`) | 古いサンプルは捨てる |
| バイナリパース | `byteorder` (LittleEndian) | Forza Data Out は LE |
| 設定 | `serde` + `toml` | `config.toml` / `profiles.toml` |
| Win32 | `windows` 0.58 | P/Invoke (クリックスルー / DPI / ホットキー) |
| ログ | `log` + `env_logger` | `RUST_LOG=info` |
| エラー | `anyhow` | アプリ層は `Result<()>` |

### Cargo フィーチャ要点

- `eframe` は `default-features = false` で `wgpu` / `default_fonts` を明示。x11/wayland は含めない。
- `windows` は `Win32_UI_HiDpi` (DPI + ホットキー系) や `Win32_Graphics_Gdi` などを feature 指定。
- 画像クレート (`image`) は現在未使用。オーバーレイは PNG 合成を使っていない。

---

## ディレクトリ構成

```
FHTHD/                       (リポジトリのルートディレクトリ名は据え置き)
├── Cargo.toml
├── Cargo.lock               (コミット対象)
├── config.toml              ローカル設定 (実行時生成・.gitignore 対象)
├── profiles.toml            車別ダイノ/ギア比プロファイル (実行時生成・.gitignore 対象)
├── README.md
├── LICENSE.md               zlib License
├── .github/
│   └── copilot-instructions.md  ← 本ファイル
├── .gitignore
├── assets/
│   └── Montserrat-Italic.ttf     埋め込みフォント (OFL, コミット対象)
└── src/
    ├── main.rs             エントリ、Config、App、主ループ。
    │                       起動時に System DPI Aware を設定し、設定 UI をメインウィンドウに、
    │                       オーバーレイを単一子ビューポートとして描画、HWND 取得後クリックスルー化。
    ├── state.rs            AppState (共有状態)、Layout、LayoutItem、DynoBuffer、
    │                       プロファイル永続化、入力アイドル検出。
    ├── platform/
    │   ├── mod.rs          OS 判定 + 非 Windows 向け stub
    │   └── windows.rs      Win32: クリックスルー、Show/Hide、FindWindowW、DPI、ホットキー等
    ├── telemetry/
    │   ├── mod.rs
    │   ├── packet.rs       Car Dash 324B パーサ (LittleEndian, DASH_OFFSET=244)
    │   └── receiver.rs     UDP 受信スレッド + ForwardLink (別ツールへ転送)
    └── ui/
        ├── mod.rs          WidgetSpec / WIDGETS レジストリを定義
        ├── editor.rs       設定パネル (メインウィンドウの中身) + ダイノグラフ
        ├── fonts.rs        Montserrat Italic の登録
        ├── stats.rs        統計 (加速度 G 等)
        ├── shift.rs        シフトインジケータ
        ├── gear.rs         ギア数値
        ├── speed.rs        速度 (km/h / mph)
        ├── input_text.rs   ACC / BRK 入力テキスト (paint_acc / paint_brk)
        └── telemetry_debug.rs  全フィールドデバッグ表示
```

> 注: `src/ui/inputs.rs.tmp` のような一時ファイルは作業残骸。参照・編集しないこと。

### アーキテクチャ概要

```
  +-------------------------+        +---------------------------------+
  | main (Settings) window  |        | single overlay viewport         |
  |  - decorated, resizable |        |  - transparent                  |
  |  - hosts editor::show() |        |  - no decorations               |
  |  - drives App::update() |        |  - always-on-top                |
  +-----------+-------------+        |  - clickthrough (EX_STYLE)      |
              |                      |  - size = bbox of visible items |
              | show_viewport_immediate (1 only)                       |
              +---------------->-----+---------------------------------+
                                     | egui::Area per widget at rel pos|
                                     |    -> paint(ui, &AppState, scale)|
                                     +---------------------------------+
```

- **オーバーレイは 1 枚に統一**: 全ての表示中ウィジェットの外接矩形 (bbox) を覆う透明子ビューポートを
  1 つ作り、各ウィジェットを相対座標で配置する。
- **bbox の 1px クランプ**: bbox がプライマリディスプレイ全体を覆いそうな場合は `display - 1px` に
  クランプし、Forza のフルスクリーンフリップ (DWM Independent Flip / MPO) を奪わない。
- **クリックスルー**は bbox オーバーレイの HWND に 1 度だけ適用。
  `platform::find_hwnd_by_title("ArkForzaWidgets-overlay")`。
- **サイズ/位置の追従**: エディタで x/y/scale を変えると毎フレ bbox を再計算し、
  `ViewportCommand::OuterPosition` / `InnerSize` で追従。
- **表示/非表示**: `AppState::should_show_overlay()` が `false`、または全ウィジェットが非表示のときは
  `show_viewport_immediate` を呼ばない → ビューポートが廃棄される (DWM 負荷ゼロ)。

### モジュール責務の原則

- `telemetry/`: ネットワーク受信とバイト→構造体の変換のみ。UI 非依存。
- `platform/`: Win32 P/Invoke ラッパーのみ。UI 非依存。
- `state.rs`: アプリ全体の状態と、テレメトリ→表示用集計の派生ロジック。UI 非依存。
- `ui/`: `state.rs` の値を読むだけ。状態を直接ミューテートしない (設定パネル上のイベントは除く)。
- `ui/mod.rs::WIDGETS`: オーバーレイに表示するウィジェットの中央レジストリ。**追加するならここに 1 エントリ足すだけ**。
- `main.rs`: 起動・設定読込・スレッド起動・`WIDGETS` から bbox を算出して 1 ウィンドウ描画。

### 新しいオーバーレイウィジェットの追加手順

1. **描画モジュール作成**: `src/ui/<name>.rs`
   ```rust
   use egui::{Ui, Vec2};
   use crate::state::AppState;
   pub const INTRINSIC_SIZE: Vec2 = Vec2::new(W, H); // scale=1.0 のときの外枠サイズ
   pub fn paint(ui: &mut Ui, state: &AppState, scale: Vec2) {
       // scale.x / scale.y で横・縦個別に拡縮。フォント/パディング/サイズをすべて *scale して描く
   }
   ```
2. **レイアウト追加**: `src/state.rs::Layout` にフィールド + `default_<name>` を追加。
3. **モジュール登録**: `src/ui/mod.rs` に `pub mod <name>;`。
4. **レジストリ追加**: `src/ui/mod.rs::WIDGETS` に `WidgetSpec { ... }` を 1 エントリ追加。
5. `main.rs` / `editor.rs` は修正不要。`WIDGETS` をループして自動でオーバーレイ配置・設定 UI 表示される。

### カスタムフォント

- `assets/Montserrat-Italic.ttf` を `include_bytes!` で exe に埋め込み、起動時に
  `ui::fonts::install(&ctx)` で `FontFamily::Name(ui::fonts::MONTSERRAT.into())` として登録。
- ライセンスは SIL Open Font License 1.1 (OFL)。商用利用・同梱・再配布いずれも可なのでコミット対象。
- 利用側: `FontId::new(size, FontFamily::Name(ui::fonts::MONTSERRAT.into()))`。

---

## Forza 側設定 (動作確認に必要)

ゲーム内 `Settings → HUD and Gameplay → Data Out`:

| 項目 | 値 |
|---|---|
| Data Out | ON |
| Data Out IP Address | `127.0.0.1` |
| Data Out IP Port | `35530` (`config.toml` の `bind` ポートと一致) |
| Data Out Packet Format | **Car Dash** (固定 324 bytes) |

`Sled` フォーマット (FM 系の 232 bytes) は現状非対応。

---

## パケット仕様 (重要)

公式: https://support.forza.net/hc/en-us/articles/51744149102611

`src/telemetry/packet.rs` の前提:

- 全体サイズ: **324 bytes**
- レイアウト: `[0..232)` Sled / `[232..244)` Horizon HUD パディング (12B) / `[244..324)` Dash 部
- `DASH_OFFSET = 244`

**トラブルシュート**: Power/Torque が常に 0、入力値がズレる場合は `DASH_OFFSET` を `244 ↔ 232` で
切替えて検証する (タイトル更新でパディングが変わる可能性)。

### 主要フィールド

| 物理量 | 型 | オフセット | 備考 |
|---|---|---|---|
| IsRaceOn | i32 | 0 | 0 = ポーズ中等 |
| EngineMaxRpm | f32 | 8 | ダイノ X 軸上限 |
| CurrentEngineRpm | f32 | 16 | |
| TireSlip[4] | f32×4 | Sled 内 | ダイノ記録ゲートに使用 |
| Speed (m/s) | f32 | 244+12 | |
| Power (W) | f32 | 244+16 | HP 換算 `/745.6999` |
| Torque (Nm) | f32 | 244+20 | |
| Accel | f32 (0..=1) | 244+71 | 0..=255 を正規化 |
| Brake | f32 (0..=1) | 244+72 | |
| Clutch | f32 (0..=1) | 244+73 | |
| Handbrake | f32 (0..=1) | 244+74 | |
| Gear | u8 | 244+75 | 0 = R |
| Steer | f32 (-1..=1) | 244+76 | -127..=127 を正規化 |

追加フィールドが必要なら `packet.rs` の `Telemetry` と `parse()` 両方を更新する。

---

## 設定ファイル

### config.toml (ローカル、`.gitignore` 対象)

既定値は実ファイルではなく **`src/main.rs::Config::default` と `src/state.rs::Layout` の各 `default_*`** で決まる。
既定値を変えるときはこの 2 箇所を合わせて更新すること。

主なキー:

| キー | 既定 | 意味 |
|---|---|---|
| `bind` | `"0.0.0.0:35530"` | UDP 受信アドレス |
| `settings_size` | `[520.0, 600.0]` | 設定ウィンドウ初期サイズ |
| `overlay_enabled` | `true` | オーバーレイ全体 ON/OFF |
| `auto_hide_when_inactive` | `true` | 対象ゲームが前面のときだけ表示 |
| `target_processes` | `["forzahorizon6.exe"]` | フォアグラウンド判定対象 |
| `gpu_preference` | `"high_performance"` | `auto` / `high_performance` / `low_power` |
| `input_text_bg_alpha` | `107` | ACC/BRK テキスト背景アルファ |
| `input_text_pad` | `6.0` | ACC/BRK テキスト背景パディング |
| `speed_unit_kph` | `true` | `true`=km/h, `false`=mph |
| `forward_enabled` | `false` | 受信パケットを別ツールへ転送 |
| `forward_target` | `"127.0.0.1:5300"` | 転送先 `IP:Port` |

### profiles.toml (ローカル、`.gitignore` 対象)

車/PI ごとのダイノカーブとギア比。`src/state.rs::load_profiles` / `save_profiles` が読み書きする。

---

## UDP パケット転送

- `telemetry/receiver.rs::ForwardLink` (`enabled: AtomicBool`, `target: Mutex<Option<SocketAddr>>`) を
  受信スレッドと共有する。
- 受信した生パケットを、`forward_enabled` が真のとき `forward_target` へ `send_to` で転送する。
- 設定変更は `App::update()` が変更検出して `ForwardLink::update()` で反映する。

---

## System DPI Aware (マルチ DPI クラッシュ対策)

- `main()` 冒頭 (ウィンドウ生成前) で `platform::set_system_dpi_aware()`
  (= `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE)`) を呼ぶ。
- **理由**: egui 0.29 は `Fonts` を `pixels_per_point` ごとにキャッシュし、全ビューポートで単一の
  フォントテクスチャ `Managed(0)` を共有する。設定ウィンドウを別 DPI モニタへ動かすとメインとオーバーレイで
  ppp が 2 種になり、`end_pass` で部分テクスチャ更新が他方のアトラスサイズを超えて
  "Partial texture update is outside the bounds" パニックを起こす (debug ビルドの `debug_assert`)。
- egui のグローバル `zoom_factor` は 1 つしかないため、native scale factor が異なる 2 窓の ppp を
  一致させる手段は「全窓を同じ scale_factor にする = System DPI Aware」しかない。
  → `vctx.set_pixels_per_point()` をオーバーレイ closure 内で呼ぶのは **禁止** (zoom フィードバックで縮小し続ける)。
- System DPI Aware 化により全窓 scale_factor がシステム DPI 固定になるので、オーバーレイ配置の ppp は
  `ctx.pixels_per_point()` をそのまま使ってよい (全 API が同一 DPI 空間で整合)。

---

## ビルド / 実行

```powershell
# 開発実行
cargo run

# コンパイル確認 (完了報告前に必須)
cargo check

# リリースビルド (配布用)
cargo build --release   # → target\release\ArkForzaWidgets.exe

# ログ詳細
$env:RUST_LOG="debug"; cargo run
```

リリースは `Cargo.toml` の `[profile.release]` で `lto="thin"`, `codegen-units=1`, `strip=true`。

### 配布物
- `target\release\ArkForzaWidgets.exe`
- `config.toml` (任意。無ければ起動時に自動生成)

---

## ホットキー

OS のグローバルホットキー (`RegisterHotKey`) で、ゲームが前面でも受け取れる。

| キー | 動作 |
|---|---|
| `Alt+S` | 現在の車のダイノ/ギア比を `profiles.toml` に保存 (未保存かつ記録データがあるときのみ) |
| `Alt+D` | 保存済みプロファイルを削除し、ライブ記録をクリア |

オーバーレイ側はクリックスルーなのでキーボード/マウスを受け取らない。

---

## 設計上の重要な約束

1. **UDP 受信スレッドは絶対にブロックさせない**
   - `crossbeam-channel` を `bounded(1)` + `try_send`、満杯時は捨てる。
   - 受信時に `egui_ctx.request_repaint()` → イベント駆動描画。
   - UI 側は `try_recv` で最新値だけ取得。

2. **再描画はイベント駆動**
   - `update()` 末尾の `request_repaint_after` はアイドル時の最低保証。
   - データ受信時は受信スレッドの `request_repaint()` で低遅延描画。

3. **`PresentMode::AutoNoVsync` + `desired_maximum_frame_latency: Some(1)`** を維持 (`vsync = false`)。
   - 低遅延優先。DWM 経路下では Vsync ありだと Forza 側 FPS を奪う傾向 (検証済)。

4. **状態の単一所有**
   - `AppState` は `Arc<Mutex<_>>` を `App` が保持。`paint(ui, &AppState, scale)` には不変参照のみ。
   - ミューテーションは設定パネル (`ui::editor::show`) と `App::update()` / `AppState::ingest()` のみ。

5. **アロケーション削減**
   - 受信バッファはスタック固定長。パーサは `byteorder` スライス読み (ゼロアロケーション)。

6. **プラットフォーム分離**
   - `cfg(windows)` が必要な P/Invoke は `platform/windows.rs` に隠蔽、`platform/mod.rs` で stub を出す。

7. **DWM Independent Flip / MPO 問題**
   - モニタ全体を覆う透明 always-on-top ウィンドウは Forza のフルスクリーンフリップを奪う。
   - **対策**: bbox がディスプレイ全体を覆いそうなとき `display - 1px` にクランプ済み。
   - **教訓**: 透明 always-on-top は構造的に数 fps の段差がある。ウィンドウ数を増やさず **bbox 1 枚構成**で固定。

8. **ダイノ記録ゲート**
   - `is_race_on` 立ち上がりで `dyno.clear()`。
   - 記録条件: `is_race_on && brake<0.02 && clutch<0.02 && max_tire_slip<1.0 && speed>5km/h`。

9. **入力アイドル自動非表示**
   - `AppState::should_show_overlay()` は accel/brake/steer の最後の入力から一定時間内を要求。
   - 何か入力があればすぐ再表示 (UDP は常時届いている前提)。

---

## コーディング規約

- `cargo fmt` / `cargo clippy` を都度実行。
- `unsafe` は P/Invoke 周辺のみ許可。それ以外は使わない。
- `unwrap()` は本当に panic させたい所のみ。受信や I/O は `?` で伝播。
- ログは `log` マクロ経由 (`println!` 禁止)。
- 新規追加した公開型・関数には `///` ドキュメントコメントを書く。

---

## エージェント運用ルール

- **過剰な抽象化を避ける**: 1 度しか呼ばれない関数を切り出さない、不要な trait を作らない。
- **要求にないリファクタリングをしない**: 頼まれた範囲だけ変える。
- **ドキュメントを勝手に増やさない**: 明示的に頼まれない限り `.md` を新規作成しない。
- **ビルド確認**: 変更後は最低限 `cargo check` を通してから完了報告する。
- **公開衛生**: `config.toml` / `profiles.toml` / `target/` は公開対象に含めない (`.gitignore` 済み)。
- **回答言語**: 日本語で返す。

---

## 参考リンク

- Forza Data Out 仕様: https://support.forza.net/hc/en-us/articles/51744149102611
- egui: https://docs.rs/egui ・ eframe: https://docs.rs/eframe
- egui_plot: https://docs.rs/egui_plot ・ wgpu: https://docs.rs/wgpu
- windows crate: https://docs.rs/windows
