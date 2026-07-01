use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use eframe::egui;
use log::{debug, warn};

use super::packet::{self, Telemetry, PACKET_SIZE};

/// 受信した生パケットの転送先設定。受信スレッドと UI スレッドで共有する。
///
/// 他のテレメトリ解析ツールへ Forza のパケットをそのまま中継するために使う。
#[derive(Default)]
pub struct ForwardLink {
    enabled: AtomicBool,
    target: Mutex<Option<SocketAddr>>,
}

impl ForwardLink {
    pub fn new() -> Self {
        Self::default()
    }

    /// UI 側から転送設定を更新する。
    /// `target_str` のパースに失敗した場合は転送しない (enabled でも無効扱い)。
    pub fn update(&self, enabled: bool, target_str: &str) {
        let addr = target_str.trim().parse::<SocketAddr>().ok();
        *self.target.lock().unwrap() = addr;
        self.enabled
            .store(enabled && addr.is_some(), Ordering::Relaxed);
    }

    /// 現在有効な転送先を取得する (無効なら None)。
    fn snapshot(&self) -> Option<SocketAddr> {
        if self.enabled.load(Ordering::Relaxed) {
            *self.target.lock().unwrap()
        } else {
            None
        }
    }
}

/// バックグラウンドスレッドで UDP を購読し、パース結果を `tx` に流す。
///
/// - `tx` は容量 1 のチャネルを推奨 (古いサンプルを捨てて常に最新値だけを描画する)。
/// - `egui_ctx` には eframe の `Context` を渡す。パケット受信ごとに
///   `request_repaint()` を呼ぶことで、次フレームを待たずに UI を即時再描画させる。
/// - `forward` が有効なときは、受信した生パケットを転送先へそのまま中継する。
pub fn spawn(
    bind_addr: &str,
    tx: Sender<Telemetry>,
    egui_ctx: egui::Context,
    forward: Arc<ForwardLink>,
) -> Result<()> {
    let socket = UdpSocket::bind(bind_addr)
        .with_context(|| format!("failed to bind UDP socket on {bind_addr}"))?;
    // ブロッキング受信を基本にしつつ、到着済みパケットの drain だけ非ブロッキングで行う
    socket.set_nonblocking(false)?;

    log::info!("UDP listening on {bind_addr}");

    thread::Builder::new()
        .name("telemetry-rx".into())
        .spawn(move || {
            let mut buf = [0u8; 1024];
            let mut tmp = [0u8; 1024];
            loop {
                // 1) まずブロッキングで 1 本受信
                let n = match socket.recv(&mut buf) {
                    Ok(n) => n,
                    Err(e) => {
                        warn!("recv error: {e}");
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }
                };
                // この反復で使う転送先を 1 度だけ取得する。
                let fwd = forward.snapshot();
                if let Some(addr) = fwd {
                    let _ = socket.send_to(&buf[..n], addr);
                }
                let mut latest_len = n;

                // 2) OS の受信キューに溜まっている古いパケットを drain して
                //    最後に届いたものだけを採用する (UI 停止明け等の遅延吸収)。
                //    転送はパケットを落とさないよう drain した分もすべて中継する。
                if socket.set_nonblocking(true).is_ok() {
                    loop {
                        match socket.recv(&mut tmp) {
                            Ok(m) => {
                                if let Some(addr) = fwd {
                                    let _ = socket.send_to(&tmp[..m], addr);
                                }
                                buf[..m].copy_from_slice(&tmp[..m]);
                                latest_len = m;
                            }
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                            Err(e) => {
                                debug!("drain recv error: {e}");
                                break;
                            }
                        }
                    }
                    let _ = socket.set_nonblocking(false);
                }

                if latest_len < PACKET_SIZE {
                    debug!("ignored short packet: {latest_len} bytes");
                    continue;
                }

                match packet::parse(&buf[..latest_len]) {
                    Ok(sample) => {
                        // 古いサンプルは捨てて最新だけ残す
                        let _ = tx.try_send(sample);
                        // UI スレッドを即起こす (次フレームを待たない)
                        egui_ctx.request_repaint();
                    }
                    Err(e) => debug!("parse error: {e}"),
                }
            }
        })
        .context("failed to spawn telemetry thread")?;

    Ok(())
}
