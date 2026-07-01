# ArkForzaWidgets

ArkForzaWidgets は、Forza Horizon 6 が UDP で出力するテレメトリを受信し、ゲーム画面上に半透明オーバーレイで補助情報を表示する Windows 向けデスクトップアプリです。

## 主な機能

- 単一の透明オーバーレイウィンドウで HUD を描画
- 速度、RPM、ギア、スロットル、ブレーキなどの数値表示
- シフトインジケータ表示
- ダイノプロファイル保存と読み出し
- 車ごとのギア比 / パワーカーブを `profiles.toml` に保存
- 受信した UDP パケットの別ツールへの転送
- グローバルホットキーでプロファイル保存 / クリア

## 動作要件

- Windows 10 / 11 (x64)
- Forza Horizon 6
- Rust 1.96.0 以降 (ソースからビルドする場合)

## 技術スタック

- Rust
- eframe / egui / wgpu
- std::net::UdpSocket
- crossbeam-channel
- serde / toml

## 実行方法

```powershell
cargo run
```

リリースビルド:

```powershell
cargo build --release
```

生成物は `target/release/ArkForzaWidgets.exe` です。

## Forza 側設定

ゲーム内の `Settings -> HUD and Gameplay -> Data Out` を開き、以下を設定してください。

| 項目 | 値 |
|---|---|
| Data Out | ON |
| Data Out IP Address | `127.0.0.1` |
| Data Out IP Port | `35530` |
| Data Out Packet Format | `Car Dash` |

`Data Out IP Port` は `config.toml` の `bind` と一致させてください。

## 設定ファイル

初回起動時に `config.toml` と `profiles.toml` が必要に応じて作成されます。

設定例:

```toml
bind = "0.0.0.0:35530"
settings_size = [520.0, 600.0]
overlay_enabled = true
auto_hide_when_inactive = true
target_processes = ["forzahorizon6.exe"]
gpu_preference = "high_performance"
input_text_bg_alpha = 107
input_text_pad = 6.0
speed_unit_kmh = true
forward_enabled = false
forward_target = "127.0.0.1:5300"
```

主な設定項目:

- `overlay_enabled`: オーバーレイ全体の ON / OFF
- `auto_hide_when_inactive`: 対象ゲームが前面のときだけ表示
- `gpu_preference`: `auto` / `high_performance` / `low_power`
- `speed_unit_kmh`: `true` で km/h、`false` で mph
- `forward_enabled`: 受信した UDP パケットを別ツールへ転送するか
- `forward_target`: 転送先 `IP:Port`

## ホットキー

| キー | 動作 |
|---|---|
| `Alt+S` | 現在の車のダイノ / ギア比プロファイルを保存 |
| `Alt+D` | 保存済みプロファイルを削除し、ライブ記録をクリア |

## 配布

配布時の最小構成:

- `target/release/ArkForzaWidgets.exe`
- `config.toml` (任意。無ければ起動時に生成)
- `assets/` (利用するアセットがある場合)

## セキュリティ / 公開時の注意

- このアプリは既定でローカル UDP (`127.0.0.1` / `0.0.0.0:35530`) を使用します。
- パケット転送は `forward_enabled = true` のときだけ有効です。
- `config.toml` と `profiles.toml` には個人の設定や走行データが入るため、公開用リポジトリに含めるかは明示的に判断してください。
- `target/` 配下の生成物にはローカル環境のパスが含まれるため、公開リポジトリには含めないでください。

## 既知の制限

- `Car Dash` (324 bytes) フォーマット前提です。`Sled` フォーマットは未対応です。
- Windows 専用です。
- カスタムフォントや画像アセットを追加する場合は、その配布条件を各自で確認してください。

## ライセンス

このプロジェクトは zlib License です。詳細は `LICENSE.md` を参照してください。
