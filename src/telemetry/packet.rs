// Forza Horizon 6 "CarDash" UDP パケット定義 (324 bytes, little-endian)
//
// 公式仕様: https://support.forza.net/hc/en-us/articles/51744149102611
//
// レイアウト概要:
//   [  0.. 232) Sled        (FM/FH 共通)
//   [232.. 244) HorizonHud  (12 bytes, Horizon タイトル専用パディング)
//   [244.. 324) Dash        (位置・ラップ・入力・パワー等)
//
// 値が明らかにおかしい場合 (Power が 0 のままなど) は、本ファイル先頭の
// 定数 `DASH_OFFSET` を 244 → 232 に変更して再ビルドしてください。
// (タイトルによっては 12 byte パディングが末尾に来る派生レイアウトがあります)

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LittleEndian as LE};

pub const PACKET_SIZE: usize = 324;
const DASH_OFFSET: usize = 244;

/// 1 サンプルぶんのテレメトリ。UI が必要とする値だけを抜粋。
#[derive(Debug, Clone, Copy, Default)]
pub struct Telemetry {
    pub is_race_on: bool,
    pub timestamp_ms: u32,

    // Engine
    pub engine_max_rpm: f32,
    pub engine_idle_rpm: f32,
    pub current_rpm: f32,

    /// 車体ローカル座標の加速度 (m/s²):
    /// X = 右, Y = 上, Z = 前 (公式仕様 Sled offset 20/24/28)
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,

    // Dash
    pub speed_mps: f32,
    pub power_w: f32,
    pub torque_nm: f32,

    // Inputs (0..=255 / -127..=127 を 0.0..=1.0 / -1.0..=1.0 に正規化済み)
    pub accel: f32,
    pub brake: f32,
    pub clutch: f32,
    pub handbrake: f32,
    pub steer: f32,
    /// 正規化ドライビングライン位置 (S8: -127..=127)
    pub normalized_driving_line: i8,
    /// 正規化 AI ブレーキ差分 (S8: -127..=127)
    pub normalized_ai_brake_difference: i8,
    pub gear: u8,

    /// 車両識別 ID (Sled offset 212)。タイトル内で車種ごとにユニーク。
    pub car_ordinal: i32,
    /// PI (Performance Index, Sled offset 220)。同一車でもチューニングで変わる。
    pub car_performance_index: i32,

    /// 4 輪の "combined slip" (Sled 部 offset 192/196/200/204)。
    /// 1.0 を超えるとタイヤがグリップを失っている目安。
    pub tire_slip: [f32; 4],
}

impl Telemetry {
    pub fn power_hp(&self) -> f32 {
        self.power_w / 745.699_9
    }

    pub fn speed_kph(&self) -> f32 {
        self.speed_mps * 3.6
    }

    /// 横 G (m/s² → G)。右方向が正。
    pub fn lateral_g(&self) -> f32 {
        self.accel_x / 9.806_65
    }

    /// 縦 G (m/s² → G)。前方向が正。
    pub fn longitudinal_g(&self) -> f32 {
        self.accel_z / 9.806_65
    }

    /// 垂直 G (m/s² → G)。上方向が正。
    pub fn vertical_g(&self) -> f32 {
        self.accel_y / 9.806_65
    }

    /// 3軸合成加速度の大きさ (G)。
    pub fn total_g(&self) -> f32 {
        let gx = self.lateral_g();
        let gy = self.vertical_g();
        let gz = self.longitudinal_g();
        (gx * gx + gy * gy + gz * gz).sqrt()
    }

    /// 4 輪のうち最大のスリップ量
    pub fn max_tire_slip(&self) -> f32 {
        self.tire_slip
            .iter()
            .cloned()
            .fold(0.0_f32, |a, b| a.max(b.abs()))
    }
}

/// UDP ペイロードを `Telemetry` にパースする。
pub fn parse(buf: &[u8]) -> Result<Telemetry> {
    if buf.len() < PACKET_SIZE {
        bail!(
            "packet too small: got {} bytes, expected >= {}",
            buf.len(),
            PACKET_SIZE
        );
    }

    // Sled
    let is_race_on = LE::read_i32(&buf[0..4]) != 0;
    let timestamp_ms = LE::read_u32(&buf[4..8]);
    let engine_max_rpm = LE::read_f32(&buf[8..12]);
    let engine_idle_rpm = LE::read_f32(&buf[12..16]);
    let current_rpm = LE::read_f32(&buf[16..20]);

    // 車体ローカル座標の加速度 (Sled offset 20/24/28)
    let accel_x = LE::read_f32(&buf[20..24]);
    let accel_y = LE::read_f32(&buf[24..28]);
    let accel_z = LE::read_f32(&buf[28..32]);

    // Tire combined slip (Sled offsets 192/196/200/204)
    let tire_slip = [
        LE::read_f32(&buf[192..196]),
        LE::read_f32(&buf[196..200]),
        LE::read_f32(&buf[200..204]),
        LE::read_f32(&buf[204..208]),
    ];

    // 車両識別 (Sled offsets: CarOrdinal=212, CarPerformanceIndex=220)
    let car_ordinal = LE::read_i32(&buf[212..216]);
    let car_performance_index = LE::read_i32(&buf[220..224]);

    // Dash 部 (DASH_OFFSET 起点)
    let d = DASH_OFFSET;
    let speed_mps = LE::read_f32(&buf[d + 12..d + 16]);
    let power_w = LE::read_f32(&buf[d + 16..d + 20]);
    let torque_nm = LE::read_f32(&buf[d + 20..d + 24]);

    // u8 / i8 入力群
    // 244 + 71 = 315: Accel から始まる
    let accel = buf[d + 71] as f32 / 255.0;
    let brake = buf[d + 72] as f32 / 255.0;
    let clutch = buf[d + 73] as f32 / 255.0;
    let handbrake = buf[d + 74] as f32 / 255.0;
    let gear = buf[d + 75];
    let steer = (buf[d + 76] as i8) as f32 / 127.0;
    let normalized_driving_line = buf[d + 77] as i8;
    let normalized_ai_brake_difference = buf[d + 78] as i8;

    Ok(Telemetry {
        is_race_on,
        timestamp_ms,
        engine_max_rpm,
        engine_idle_rpm,
        current_rpm,
        accel_x,
        accel_y,
        accel_z,
        speed_mps,
        power_w,
        torque_nm,
        accel,
        brake,
        clutch,
        handbrake,
        steer,
        normalized_driving_line,
        normalized_ai_brake_difference,
        gear,
        car_ordinal,
        car_performance_index,
        tire_slip,
    })
}
