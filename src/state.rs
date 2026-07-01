use serde::{Deserialize, Deserializer, Serialize};

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::telemetry::Telemetry;

/// 旧 `scale = 1.0` (単一 float) を読みつつ、新フォーマット `scale = [1.0, 1.0]` も受け付ける。
fn de_scale_xy<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 2], D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum X {
        Pair([f32; 2]),
        Single(f32),
    }
    Ok(match X::deserialize(d)? {
        X::Pair(p) => p,
        X::Single(s) => [s, s],
    })
}

fn default_scale_xy() -> [f32; 2] {
    [1.0, 1.0]
}

/// 1 ウィジェットの配置情報。`config.toml` に永続化される。
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LayoutItem {
    pub visible: bool,
    /// ゲーム解像度に対する **重心 (中心) 位置** を百分率 0.0..=1.0 で表す。
    /// `[0.5, 0.5]` で画面中央。実ピクセル座標は描画時に
    /// `origin + pos * resolution - widget_size / 2` で求める。
    pub pos: [f32; 2],
    /// 横/縦それぞれの倍率 (1.0 = 等倍)
    #[serde(deserialize_with = "de_scale_xy", default = "default_scale_xy")]
    pub scale: [f32; 2],
}

impl LayoutItem {
    pub fn new(pos: [f32; 2]) -> Self {
        Self {
            visible: true,
            pos,
            scale: [1.0, 1.0],
        }
    }
}

/// 全ウィジェットの配置をまとめた構造体。新しいウィジェットを追加したらフィールドを足す。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default = "Layout::default_stats")]
    pub stats: LayoutItem,
    #[serde(default = "Layout::default_shift")]
    pub shift: LayoutItem,
    #[serde(default = "Layout::default_acc_text")]
    pub acc_text: LayoutItem,
    #[serde(default = "Layout::default_brk_text")]
    pub brk_text: LayoutItem,
    #[serde(default = "Layout::default_gear_display")]
    pub gear_display: LayoutItem,
    #[serde(default = "Layout::default_speed_display")]
    pub speed_display: LayoutItem,
    #[serde(default = "Layout::default_telemetry_debug")]
    pub telemetry_debug: LayoutItem,
}

impl Layout {
    fn default_stats() -> LayoutItem {
        LayoutItem {
            visible: true,
            pos: [0.8999999761581421, 0.5960000157356262],
            scale: [1.0, 0.5],
        }
    }
    fn default_shift() -> LayoutItem {
        LayoutItem {
            visible: true,
            pos: [0.5, 0.800000011920929],
            scale: [1.5, 1.0],
        }
    }
    fn default_acc_text() -> LayoutItem {
        LayoutItem {
            visible: true,
            pos: [0.6499999761581421, 0.7099999785423279],
            scale: [1.5, 1.5],
        }
    }
    fn default_brk_text() -> LayoutItem {
        LayoutItem {
            visible: true,
            pos: [0.3499999940395355, 0.7099999785423279],
            scale: [1.5, 1.5],
        }
    }
    fn default_gear_display() -> LayoutItem {
        LayoutItem {
            visible: true,
            pos: [0.35499998927116394, 0.800000011920929],
            scale: [1.5, 1.5],
        }
    }
    fn default_speed_display() -> LayoutItem {
        LayoutItem {
            visible: true,
            pos: [0.6690000295639038, 0.800000011920929],
            scale: [1.5, 1.5],
        }
    }
    fn default_telemetry_debug() -> LayoutItem {
        LayoutItem {
            visible: false,
            pos: [0.6790000200271606, 0.19499999284744263],
            scale: [1.0, 1.0],
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            stats: Self::default_stats(),
            shift: Self::default_shift(),
            acc_text: Self::default_acc_text(),
            brk_text: Self::default_brk_text(),
            gear_display: Self::default_gear_display(),
            speed_display: Self::default_speed_display(),
            telemetry_debug: Self::default_telemetry_debug(),
        }
    }
}

/// ダイノグラフ用: RPM ビンごとに観測した Power/Torque の最大値を保持する。
/// 走るほど "実走ダイノカーブ" が浮かび上がる。
pub struct DynoBuffer {
    pub bin_rpm: f32, // 1 ビンあたりの RPM 幅
    pub max_rpm: f32, // 上限 (これ以上は最後のビンへ)
    pub power: Vec<f32>,
    pub torque: Vec<f32>,
}

impl DynoBuffer {
    pub fn new(bin_rpm: f32, max_rpm: f32) -> Self {
        let n = (max_rpm / bin_rpm).ceil() as usize + 1;
        Self {
            bin_rpm,
            max_rpm,
            power: vec![0.0; n],
            torque: vec![0.0; n],
        }
    }

    pub fn update(&mut self, rpm: f32, power_hp: f32, torque_nm: f32) {
        if rpm <= 0.0 || !rpm.is_finite() {
            return;
        }
        let idx = ((rpm / self.bin_rpm) as usize).min(self.power.len() - 1);
        if power_hp > self.power[idx] {
            self.power[idx] = power_hp;
        }
        if torque_nm > self.torque[idx] {
            self.torque[idx] = torque_nm;
        }
    }

    pub fn clear(&mut self) {
        self.power.iter_mut().for_each(|v| *v = 0.0);
        self.torque.iter_mut().for_each(|v| *v = 0.0);
    }

    /// (rpm, power) の点列
    pub fn power_series(&self) -> Vec<[f64; 2]> {
        series_from(&self.power, self.bin_rpm)
    }

    pub fn torque_series(&self) -> Vec<[f64; 2]> {
        series_from(&self.torque, self.bin_rpm)
    }

    /// どのビンにもデータがなければ false
    pub fn has_data(&self) -> bool {
        self.power.iter().any(|&p| p > 0.0)
    }

    /// パワーピークの ratio 以上が連続する RPM 範囲 (start, end)
    pub fn power_band(&self, ratio: f32) -> Option<(f32, f32)> {
        power_band_from(&self.power, self.bin_rpm, ratio)
    }
}

/// `values[i]` (i 番目の RPM ビン) を (rpm, value) の点列に変換 (0 は除外)。
fn series_from(values: &[f32], bin_rpm: f32) -> Vec<[f64; 2]> {
    values
        .iter()
        .enumerate()
        .filter(|(_, &v)| v > 0.0)
        .map(|(i, &v)| [(i as f32 * bin_rpm) as f64, v as f64])
        .collect()
}

/// パワーピークの ratio 以上が連続する RPM 範囲 (start, end)。
fn power_band_from(power: &[f32], bin_rpm: f32, ratio: f32) -> Option<(f32, f32)> {
    let peak = power.iter().cloned().fold(0.0_f32, f32::max);
    if peak <= 0.0 {
        return None;
    }
    let threshold = peak * ratio;
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for (i, &p) in power.iter().enumerate() {
        if p >= threshold {
            start.get_or_insert(i);
            end = Some(i);
        }
    }
    match (start, end) {
        (Some(s), Some(e)) => Some((s as f32 * bin_rpm, e as f32 * bin_rpm)),
        _ => None,
    }
}

/// 車/PI の組み合わせごとに保存されるダイノピークとパワーバンド設定。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CarProfile {
    pub bin_rpm: f32,
    pub max_rpm: f32,
    pub power: Vec<f32>,
    pub torque: Vec<f32>,
    /// パワーバンド閾値 (0.90..=0.99)
    pub band_ratio: f32,
    /// 推定レブリミット (保存時に推定できていれば Some)
    #[serde(default)]
    pub rev_limit: Option<f32>,
    /// シフトインジケーターの左端比率 (rev_limit に対する割合、0.80 = 80%)
    #[serde(default = "default_shift_lo_ratio")]
    pub shift_lo_ratio: f32,
    /// ギアごとの「rpm / 車速(km/h)」比 (= 総減速比に比例)。
    /// index = ギア番号 (0 は未使用、1.. が前進ギア)。値 0.0 = 未記録。
    /// CarID×PI で固定なのでプロファイルに保存する。
    #[serde(default)]
    pub gear_ratios: Vec<f32>,
}

fn default_shift_lo_ratio() -> f32 {
    0.80
}

impl CarProfile {
    pub fn power_series(&self) -> Vec<[f64; 2]> {
        series_from(&self.power, self.bin_rpm)
    }

    pub fn torque_series(&self) -> Vec<[f64; 2]> {
        series_from(&self.torque, self.bin_rpm)
    }

    pub fn power_band(&self) -> Option<(f32, f32)> {
        power_band_from(&self.power, self.bin_rpm, self.band_ratio)
    }

    /// 任意の閾値でパワーバンドを計算 (スライダー追従プレビュー用)。
    pub fn power_band_with(&self, ratio: f32) -> Option<(f32, f32)> {
        power_band_from(&self.power, self.bin_rpm, ratio)
    }

    /// 指定ギアの減速比 (rpm/車速)。未記録なら None。
    pub fn gear_ratio(&self, gear: u8) -> Option<f32> {
        self.gear_ratios
            .get(gear as usize)
            .copied()
            .filter(|&v| v > 0.0)
    }

    /// 記録済みの前進ギア数 (連続して埋まっている最大ギア)。
    pub fn recorded_gear_count(&self) -> usize {
        let mut n = 0;
        for g in 1..self.gear_ratios.len() {
            if self.gear_ratio(g as u8).is_some() {
                n = g;
            }
        }
        n
    }

    /// 任意 RPM のトルク (Nm) を線形補間で取得。範囲外/データ無しは 0.0。
    fn torque_at(&self, rpm: f32) -> f32 {
        if rpm <= 0.0 || self.bin_rpm <= 0.0 || self.torque.is_empty() {
            return 0.0;
        }
        let pos = rpm / self.bin_rpm;
        let i = pos.floor() as usize;
        if i + 1 >= self.torque.len() {
            return *self.torque.last().unwrap_or(&0.0);
        }
        let frac = pos - i as f32;
        let a = self.torque[i];
        let b = self.torque[i + 1];
        a + (b - a) * frac
    }

    /// 現ギア `gear` から次ギアへの **最適シフトアップ RPM**。
    ///
    /// 車輪推進力 `torque(rpm) × 総減速比` が現ギアと次ギアで等しくなる
    /// (= 次ギアの方が強くなり始める) 交点を返す。`rev_limit` でクランプ。
    /// ギア比・トルクデータが揃っていなければ None。
    pub fn optimal_shift_rpm(&self, gear: u8, rev_limit: f32) -> Option<f32> {
        let kg = self.gear_ratio(gear)?;
        let kn = self.gear_ratio(gear + 1)?;
        // 次ギアは必ず背高 (rpm/速度 が小さい)。逆なら計算不能。
        if kg <= 0.0 || kn <= 0.0 || kn >= kg {
            return None;
        }
        if rev_limit <= 0.0 {
            return None;
        }
        // 現ギア rpm r でシフトすると次ギアは r2 = r * kn/kg。
        // f(r) = force_next - force_current = torque(r2)*kn - torque(r)*kg。
        //   f > 0 → 次ギアの方が強い (シフトすべき)
        //   f < 0 → 現ギアの方が強い (まだ引っ張る)
        let ratio = kn / kg;
        let f = |r: f32| self.torque_at(r * ratio) * kn - self.torque_at(r) * kg;
        let step = (self.bin_rpm * 0.5).max(20.0);
        let lo = (rev_limit * 0.30).max(self.bin_rpm);
        // redline でも現ギアの方が強ければ (f < 0)、シフトせず redline まで引っ張る。
        if f(rev_limit) < 0.0 {
            return Some(rev_limit);
        }
        // 低回転域のトルクはノイジーで偽の交点を生むため、redline 側から下げていき
        // f が「正 → 負」に変わる最も高い rpm を最適シフト点とする。
        let mut hi_r = rev_limit;
        let mut hi_f = f(rev_limit); // >= 0
        let mut r = rev_limit - step;
        while r >= lo {
            let cur_f = f(r);
            if cur_f < 0.0 {
                // [r, hi_r] の間で交差。線形補間で交点 rpm を求める。
                let t = (-cur_f) / (hi_f - cur_f);
                return Some(r + (hi_r - r) * t);
            }
            hi_r = r;
            hi_f = cur_f;
            r -= step;
        }
        // lo まで下げても f >= 0 (次ギアが常に強い) → lo を返す
        Some(lo)
    }

    /// 現ギア `gear` から下のギア (`gear-1`) への **最適シフトダウン RPM**。
    ///
    /// `gear-1 → gear` のアップシフト点 (下ギアの回転数) を、現ギアの回転数に
    /// 換算した値を返す。1 速など下が無い場合や、ギア比が揃わない場合は None。
    pub fn optimal_downshift_rpm(&self, gear: u8, rev_limit: f32) -> Option<f32> {
        if gear < 2 {
            return None;
        }
        let lower = gear - 1;
        // 下ギアでアップシフトすべき rpm (下ギアの回転数)
        let up_lower = self.optimal_shift_rpm(lower, rev_limit)?;
        let k_lower = self.gear_ratio(lower)?;
        let k_cur = self.gear_ratio(gear)?;
        if k_lower <= 0.0 {
            return None;
        }
        // 同一車速で下ギア up_lower rpm のとき、現ギアの rpm = up_lower * k_cur / k_lower
        Some(up_lower * k_cur / k_lower)
    }
}

/// `car_ordinal` / `car_performance_index` からプロファイルキー文字列を作る。
pub fn car_key(ordinal: i32, pi: i32) -> String {
    format!("{}-{}", ordinal, pi)
}

/// profiles.toml のルート。
#[derive(Serialize, Deserialize, Default)]
struct ProfileStore {
    #[serde(default)]
    profiles: HashMap<String, CarProfile>,
}

/// profiles.toml を読む (無ければ空)。
pub fn load_profiles(path: &Path) -> HashMap<String, CarProfile> {
    match std::fs::read_to_string(path) {
        Ok(s) => match toml::from_str::<ProfileStore>(&s) {
            Ok(store) => store.profiles,
            Err(e) => {
                log::warn!("profiles.toml parse error: {e}; ignoring");
                HashMap::new()
            }
        },
        Err(_) => HashMap::new(),
    }
}

/// profiles.toml に書き出す。
pub fn save_profiles(path: &Path, profiles: &HashMap<String, CarProfile>) -> Result<()> {
    let store = ProfileStore {
        profiles: profiles.clone(),
    };
    let s = toml::to_string_pretty(&store).context("serialize profiles")?;
    std::fs::write(path, s).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// アプリ全体で共有する状態。
pub struct AppState {
    pub latest: Telemetry,
    pub dyno: DynoBuffer,
    pub layout: Layout,
    /// ユーザーがチェックボックスで切り替える「オーバーレイ表示 ON/OFF」
    pub overlay_enabled: bool,
    /// ターゲットゲームがフォアグラウンドのときだけ表示するか
    pub auto_hide_when_inactive: bool,
    /// 直近のフォアグラウンド判定結果 (描画判定用)
    pub target_in_foreground: bool,
    /// ゲームウィンドウから検出したレンダリング解像度 (クライアント領域)。
    /// ウィジェット位置はこの解像度に対する百分率で指定される。
    /// 未検出の間は None (プライマリディスプレイにフォールバック)。
    pub game_resolution: Option<(u32, u32)>,
    /// ゲームウィンドウのクライアント領域左上のスクリーン座標 (オーバーレイ原点)。
    pub game_origin: (i32, i32),
    /// 直前フレームの is_race_on (エッジ検出用、シリアライズ対象外)
    prev_race_on: bool,
    /// 現在 wgpu が使っているアダプタの説明文字列 (起動時にセット)
    pub active_gpu: String,
    /// GPU 選択ヒント (次回起動時に適用): "auto" | "high_performance" | "low_power"
    pub gpu_preference: String,
    /// 推定レブリミット (フューエルカット作動 RPM)。is_race_on 立ち上がりで None にリセット。
    pub rev_limit: Option<f32>,
    /// 全開時の直近 RPM サンプル (張り付き検出用リングバッファ)
    recent_rpm_at_wot: [f32; REV_LIMIT_WINDOW],
    recent_rpm_idx: usize,
    recent_rpm_filled: usize,
    /// ライブ記録中のギア比 (rpm/車速)。index = ギア番号。値 0.0 = 未記録。
    /// is_race_on 立ち上がりでリセットし、保存時にプロファイルへ書き出す。
    gear_ratios_live: [f32; MAX_GEARS],
    /// 車/PI ごとに保存されたダイノプロファイル (key = "ordinal-pi")
    pub profiles: HashMap<String, CarProfile>,
    /// 未保存車用のパワーバンド閾値 (スライダー作業値)。
    pub band_ratio: f32,
    /// 最後に観測した非ゼロの車識別 (ポーズ中は car_ordinal が 0 になるため保持)
    pub last_car_ordinal: i32,
    pub last_car_pi: i32,
    /// ACC/BRK テキストウィジェットの背景矩形アルファ (0=完全透明, 255=不透明)
    pub input_text_bg_alpha: u8,
    /// ACC/BRK テキストウィジェットの追加パディング (px, scale=1.0 基準)
    pub input_text_pad: f32,
    /// 速度表示ウィジェットの単位: true = km/h, false = mph
    pub speed_unit_kmh: bool,
    /// UDP 受信ポート (変更はアプリ再起動で有効化)
    pub udp_port: u16,
    /// 受信した生パケットを他ツールへ転送するか
    pub forward_enabled: bool,
    /// 転送先 "IP:Port"
    pub forward_target: String,
    /// 直前のパケットで受信した非ゼロの engine_max_rpm。
    /// レース開始時のダイノバッファリサイズのフォールバック値として使用。
    pub last_engine_max_rpm: f32,
    /// 加速度成分の指数移動平均 (m/s²)。X=右, Y=上, Z=前。
    /// magnitude を取る前に成分を平滑化することで、ノイズ整流バイアスを抑える。
    pub accel_ema: [f32; 3],
    /// accel_ema を初期化済みか (最初のサンプルは生値で seed する)
    accel_ema_init: bool,
}

/// レブリミット張り付き検出: 全開時の直近何サンプルを見るか
const REV_LIMIT_WINDOW: usize = 8;
/// 全開判定 (アクセル踏み込み量)
const REV_LIMIT_WOT_THRESHOLD: f32 = 0.98;
/// 「張り付き」とみなす window 内の RPM 振れ幅
const REV_LIMIT_STUCK_SPREAD: f32 = 80.0;
/// 張り付きを採用する最低 RPM: EngineMaxRpm の何割以上
const REV_LIMIT_MIN_RATIO: f32 = 0.85;

/// ギア比を記録する配列サイズ (index 0 は未使用、最大10速まで)
const MAX_GEARS: usize = 11;
/// ギア比記録の最低車速 (km/h)。低速は rpm/速度 が不安定なので除外。
const GEAR_RATIO_MIN_SPEED_KPH: f32 = 30.0;
/// ギア比 EMA のスムージング係数
const GEAR_RATIO_ALPHA: f32 = 0.25;

/// 加速度成分 EMA のスムージング係数 (0..1、小さいほど強い平滑化)。
/// ゲーム内 G メーターのダンパー挙動に近づけるための値。
const ACCEL_EMA_ALPHA: f32 = 0.15;

/// 重力加速度 (m/s²)
const GRAVITY: f32 = 9.806_65;

impl Default for AppState {
    fn default() -> Self {
        Self {
            latest: Telemetry::default(),
            dyno: DynoBuffer::new(100.0, 20_000.0),
            layout: Layout::default(),
            overlay_enabled: true,
            auto_hide_when_inactive: true,
            target_in_foreground: false,
            game_resolution: None,
            game_origin: (0, 0),
            prev_race_on: false,
            active_gpu: String::new(),
            gpu_preference: "auto".to_string(),
            rev_limit: None,
            recent_rpm_at_wot: [0.0; REV_LIMIT_WINDOW],
            recent_rpm_idx: 0,
            recent_rpm_filled: 0,
            gear_ratios_live: [0.0; MAX_GEARS],
            profiles: HashMap::new(),
            band_ratio: 0.95,
            last_car_ordinal: 0,
            last_car_pi: 0,
            input_text_bg_alpha: 107,
            input_text_pad: 6.0,
            speed_unit_kmh: true,
            udp_port: 35530,
            forward_enabled: false,
            forward_target: "127.0.0.1:5300".to_string(),
            last_engine_max_rpm: 0.0,
            accel_ema: [0.0; 3],
            accel_ema_init: false,
        }
    }
}

impl AppState {
    /// 平滑化済み横 G (右方向が正)。
    pub fn smoothed_lateral_g(&self) -> f32 {
        self.accel_ema[0] / GRAVITY
    }

    /// 平滑化済み垂直 G (上方向が正)。
    pub fn smoothed_vertical_g(&self) -> f32 {
        self.accel_ema[1] / GRAVITY
    }

    /// 平滑化済み縦 G (前方向が正)。
    pub fn smoothed_longitudinal_g(&self) -> f32 {
        self.accel_ema[2] / GRAVITY
    }

    /// 平滑化済み成分から計算した 3 軸合成 G。
    pub fn smoothed_total_g(&self) -> f32 {
        let g = [
            self.accel_ema[0] / GRAVITY,
            self.accel_ema[1] / GRAVITY,
            self.accel_ema[2] / GRAVITY,
        ];
        (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt()
    }

    /// 実際にオーバーレイを描画/表示すべきか
    pub fn should_show_overlay(&self) -> bool {
        if !self.overlay_enabled {
            return false;
        }
        if !self.latest.is_race_on {
            return false;
        }
        if self.auto_hide_when_inactive && !self.target_in_foreground {
            return false;
        }
        true
    }

    /// 現在の車/PI からプロファイルキー。車未検出 (ordinal == 0) なら None。
    /// ポーズ中はテレメトリが 0 になるため、最後に観測した車識別を使う。
    pub fn current_car_key(&self) -> Option<String> {
        if self.last_car_ordinal != 0 {
            Some(car_key(self.last_car_ordinal, self.last_car_pi))
        } else {
            None
        }
    }

    /// 現在の車に対応する保存済みプロファイル。
    pub fn current_profile(&self) -> Option<&CarProfile> {
        self.current_car_key().and_then(|k| self.profiles.get(&k))
    }

    /// 現在の車に保存済みプロファイルがあるか。
    pub fn has_current_profile(&self) -> bool {
        self.current_car_key()
            .map_or(false, |k| self.profiles.contains_key(&k))
    }

    /// 現在のダイノバッファとバンド閾値を現在の車のプロファイルとして保存する。
    /// 保存したキーを返す (車未検出なら None)。
    pub fn save_current_profile(&mut self) -> Option<String> {
        let key = self.current_car_key()?;
        let existing = self.profiles.get(&key);
        // ギア比: ライブ記録を優先し、未記録ギアは既存プロファイル値を引き継ぐ。
        let mut gear_ratios = vec![0.0_f32; MAX_GEARS];
        for g in 0..MAX_GEARS {
            let live = self.gear_ratios_live[g];
            let prev = existing
                .and_then(|p| p.gear_ratios.get(g).copied())
                .unwrap_or(0.0);
            gear_ratios[g] = if live > 0.0 { live } else { prev };
        }
        // 末尾の 0.0 を切り詰める
        while gear_ratios.len() > 1 && *gear_ratios.last().unwrap() == 0.0 {
            gear_ratios.pop();
        }
        let profile = CarProfile {
            bin_rpm: self.dyno.bin_rpm,
            max_rpm: self.dyno.max_rpm,
            power: self.dyno.power.clone(),
            torque: self.dyno.torque.clone(),
            band_ratio: self.band_ratio,
            rev_limit: self.rev_limit,
            shift_lo_ratio: existing
                .map(|p| p.shift_lo_ratio)
                .unwrap_or_else(default_shift_lo_ratio),
            gear_ratios,
        };
        self.profiles.insert(key.clone(), profile);
        Some(key)
    }

    /// ライブ記録済みの前進ギア数 (連続して埋まっている最大ギア)。
    pub fn live_recorded_gear_count(&self) -> usize {
        let mut n = 0;
        for g in 1..MAX_GEARS {
            if self.gear_ratios_live[g] > 0.0 {
                n = g;
            }
        }
        n
    }

    /// ライブ記録中の指定ギアの減速比 (rpm/車速)。未記録なら None。
    pub fn live_gear_ratio(&self, gear: u8) -> Option<f32> {
        self.gear_ratios_live
            .get(gear as usize)
            .copied()
            .filter(|&v| v > 0.0)
    }

    /// 現在の車の保存済みプロファイルを削除する。
    pub fn delete_current_profile(&mut self) {
        if let Some(key) = self.current_car_key() {
            self.profiles.remove(&key);
        }
    }

    /// ライブのダイノバッファとギア比記録をクリアする。
    /// `is_race_on` 立ち上がりと同等のリセットをホットキーから手動で実行したいときに使う。
    pub fn clear_live_data(&mut self) {
        self.dyno.clear();
        self.gear_ratios_live = [0.0; MAX_GEARS];
        self.rev_limit = None;
        self.recent_rpm_idx = 0;
        self.recent_rpm_filled = 0;
        log::info!("live dyno + gear ratios cleared by user");
    }
}

impl AppState {
    pub fn ingest(&mut self, t: Telemetry) {
        // is_race_on が OFF → ON に切り替わったらダイノ・レブリミット推定をリセット
        if t.is_race_on && !self.prev_race_on {
            // 優先度: (1) 現パケットの engine_max_rpm → (2) 直前に受信した値 → (3) 20_000 フォールバック
            let new_max = if t.engine_max_rpm > 0.0 {
                (t.engine_max_rpm * 1.1).ceil().max(20_000.0)
            } else if self.last_engine_max_rpm > 0.0 {
                (self.last_engine_max_rpm * 1.1).ceil().max(20_000.0)
            } else {
                20_000.0
            };
            if (new_max - self.dyno.max_rpm).abs() > 500.0 {
                // 容量が大きく変わるときだけ再アロケート
                self.dyno = DynoBuffer::new(self.dyno.bin_rpm, new_max);
                log::info!("dyno buffer resized: max_rpm = {new_max}");
            } else {
                self.dyno.clear();
            }
            self.rev_limit = None;
            self.recent_rpm_idx = 0;
            self.recent_rpm_filled = 0;
            self.gear_ratios_live = [0.0; MAX_GEARS];
            log::info!("is_race_on rising edge: dyno reset");
        }
        self.prev_race_on = t.is_race_on;
        // 非ゼロの engine_max_rpm を常に記録しておく (次回レース開始時のフォールバック用)
        if t.engine_max_rpm > 0.0 {
            self.last_engine_max_rpm = t.engine_max_rpm;
        }
        // 加速度成分の EMA を更新 (magnitude を取る前に成分平滑化してノイズ整流バイアスを抑える)
        let raw = [t.accel_x, t.accel_y, t.accel_z];
        if self.accel_ema_init {
            for i in 0..3 {
                self.accel_ema[i] += ACCEL_EMA_ALPHA * (raw[i] - self.accel_ema[i]);
            }
        } else {
            self.accel_ema = raw;
            self.accel_ema_init = true;
        }
        self.latest = t;
        // 非ゼロの車識別を保持 (ポーズ中に 0 になっても直前の車を覚えておく)
        if t.car_ordinal != 0 {
            self.last_car_ordinal = t.car_ordinal;
            self.last_car_pi = t.car_performance_index;
        }
        // Dyno への記録は:
        //   - レース中 (is_race_on)
        //   - ブレーキを踏んでいない (negative torque で歪むのを防ぐ)
        //   - クラッチが完全に繋がっている (半クラ中は RPM と出力が連動しないため除外)
        //   - タイヤがスリップしていない (max combined slip < SLIP_THRESHOLD)
        // を全て満たすときのみ。これで純粋な加速時の出力カーブだけを残せる。
        const SLIP_THRESHOLD: f32 = 1.0;
        const BRAKE_THRESHOLD: f32 = 0.02;
        const CLUTCH_THRESHOLD: f32 = 0.02; // 少しでも抜けていたら除外
                                            // 保存済みプロファイルがある車は動的記録しない (保存内容を固定表示)。
        let has_profile = self.has_current_profile();
        // クリーンな加速サンプルか (ダイノ記録・レブリミットピンの共通ゲート)。
        let dyno_gate = !has_profile
            && t.is_race_on
            && t.brake < BRAKE_THRESHOLD
            && t.clutch < CLUTCH_THRESHOLD
            && t.max_tire_slip() < SLIP_THRESHOLD
            && t.speed_kph() > 5.0; // 低速でのノイズを除外
        if dyno_gate {
            self.dyno.update(t.current_rpm, t.power_hp(), t.torque_nm);
        }

        // ギア比 (rpm/車速) の記録。
        // ダイノ記録と同じクリーンゲートに加え、十分な車速がある前進ギアのみ。
        // ホイールスピン中は比がズレるため slip ゲートが効いている dyno_gate を流用。
        let speed_kph = t.speed_kph();
        if dyno_gate
            && (t.gear as usize) >= 1
            && (t.gear as usize) < MAX_GEARS
            && speed_kph > GEAR_RATIO_MIN_SPEED_KPH
            && t.current_rpm > 0.0
        {
            let k = t.current_rpm / speed_kph;
            let slot = &mut self.gear_ratios_live[t.gear as usize];
            *slot = if *slot <= 0.0 {
                k
            } else {
                *slot + (k - *slot) * GEAR_RATIO_ALPHA
            };
        }

        // レブリミット推定
        // - 案 A: 全開 (accel ≥ 0.98, clutch < 0.02) 時の current_rpm の最大値
        // - 案 B: 同条件で直近 N サンプルが狭い範囲に張り付いていればその時点で確定
        if t.is_race_on
            && t.accel >= REV_LIMIT_WOT_THRESHOLD
            && t.clutch < CLUTCH_THRESHOLD
            && t.current_rpm > 0.0
        {
            let prev_rev_limit = self.rev_limit;
            // 案 A: 単純最大
            if self.rev_limit.map_or(true, |r| t.current_rpm > r) {
                self.rev_limit = Some(t.current_rpm);
            }
            // 案 B: 張り付き検出
            self.recent_rpm_at_wot[self.recent_rpm_idx] = t.current_rpm;
            self.recent_rpm_idx = (self.recent_rpm_idx + 1) % REV_LIMIT_WINDOW;
            self.recent_rpm_filled = (self.recent_rpm_filled + 1).min(REV_LIMIT_WINDOW);
            if self.recent_rpm_filled == REV_LIMIT_WINDOW {
                let max = self
                    .recent_rpm_at_wot
                    .iter()
                    .cloned()
                    .fold(f32::MIN, f32::max);
                let min = self
                    .recent_rpm_at_wot
                    .iter()
                    .cloned()
                    .fold(f32::MAX, f32::min);
                let min_required = t.engine_max_rpm.max(1.0) * REV_LIMIT_MIN_RATIO;
                if max - min < REV_LIMIT_STUCK_SPREAD && max >= min_required {
                    let confirmed = max.max(self.rev_limit.unwrap_or(0.0));
                    self.rev_limit = Some(confirmed);
                }
            }

            // ダイノカーブを redline まで延ばす:
            // rev_limit が更新 (= より高い rpm に到達) した瞬間、その rpm ビンへ
            // 現在の Power/Torque をピンする。レッドライン張り付き時はホイールスピン
            // (slip ≥ 1.0) で通常記録が止まりがちだが、Forza の Power/Torque は
            // エンジン出力値なのでスリップ中でも有効。ブレーキ中だけは除外する。
            if !has_profile && t.brake < BRAKE_THRESHOLD {
                if let Some(limit) = self.rev_limit {
                    if prev_rev_limit.map_or(true, |p| limit > p) {
                        self.dyno.update(limit, t.power_hp(), t.torque_nm);
                    }
                }
            }
        } else {
            // 全開を解除したらバッファを捨てる (シフト中等の混入を防ぐ)
            self.recent_rpm_idx = 0;
            self.recent_rpm_filled = 0;
        }
    }
}
