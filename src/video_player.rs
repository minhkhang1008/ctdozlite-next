//! Bounded in-app playback for downloaded Zalo videos.
//!
//! FFmpeg decodes on native threads. At most one RGBA frame may wait for the UI, while audio
//! uses a two-second ring buffer. Starting another video or closing the modal stops both decoder
//! processes, so decoder state never accumulates across messages.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

const VIDEO_WIDTH: usize = 960;
const VIDEO_HEIGHT: usize = 540;
const FRAME_BYTES: usize = VIDEO_WIDTH * VIDEO_HEIGHT * 4;

struct ActivePlayback {
    generation: u64,
    stop: Arc<AtomicBool>,
    frame_pending: Arc<AtomicBool>,
    volume_bits: Arc<AtomicU32>,
}

static ACTIVE: Mutex<Option<ActivePlayback>> = Mutex::new(None);

struct ActiveAudioPlayback {
    generation: u64,
    message: String,
    stop: Arc<AtomicBool>,
    started_at: Instant,
}

static ACTIVE_AUDIO: Mutex<Option<ActiveAudioPlayback>> = Mutex::new(None);

pub fn stop(generation: u64) {
    if let Ok(mut active) = ACTIVE.lock() {
        if active
            .as_ref()
            .is_some_and(|playback| playback.generation == generation)
        {
            if let Some(playback) = active.take() {
                playback.stop.store(true, Ordering::Release);
            }
        }
    }
}

pub fn stop_all() {
    if let Ok(mut active) = ACTIVE.lock() {
        if let Some(playback) = active.take() {
            playback.stop.store(true, Ordering::Release);
        }
    }
}

pub fn set_volume(generation: u64, volume: f32) {
    if let Ok(active) = ACTIVE.lock() {
        if let Some(playback) = active
            .as_ref()
            .filter(|playback| playback.generation == generation)
        {
            playback
                .volume_bits
                .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Release);
        }
    }
}

/// Return the message currently playing through the in-app voice player.
pub fn active_audio_message() -> Option<String> {
    ACTIVE_AUDIO
        .lock()
        .ok()
        .and_then(|active| active.as_ref().map(|playback| playback.message.clone()))
}

/// Elapsed wall-clock time for the active voice message.
pub fn active_audio_elapsed(message: &str) -> Option<Duration> {
    ACTIVE_AUDIO.lock().ok().and_then(|active| {
        active
            .as_ref()
            .filter(|playback| playback.message == message)
            .map(|playback| playback.started_at.elapsed())
    })
}

/// Stop the active voice message when it matches `message`.
pub fn stop_audio(message: &str) {
    if let Ok(mut active) = ACTIVE_AUDIO.lock() {
        if active
            .as_ref()
            .is_some_and(|playback| playback.message == message)
        {
            if let Some(playback) = active.take() {
                playback.stop.store(true, Ordering::Release);
            }
        }
    }
}

/// Play one downloaded voice message through the default output device.
///
/// Starting another message cancels the previous decoder. Audio remains bounded by the same
/// two-second ring buffer used for video playback.
pub fn start_audio(
    path: PathBuf,
    generation: u64,
    message: String,
    gui_tx: crate::gui::GuiTx,
    egui_ctx: egui::Context,
) {
    let stop = Arc::new(AtomicBool::new(false));
    if let Ok(mut active) = ACTIVE_AUDIO.lock() {
        if let Some(previous) = active.replace(ActiveAudioPlayback {
            generation,
            message: message.clone(),
            stop: stop.clone(),
            started_at: Instant::now(),
        }) {
            previous.stop.store(true, Ordering::Release);
        }
    }
    thread::Builder::new()
        .name("zalo-voice-player".into())
        .spawn(move || {
            let result = play_audio(
                path,
                stop.clone(),
                0.0,
                Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            );
            let cancelled = stop.load(Ordering::Acquire);
            let is_current = ACTIVE_AUDIO
                .lock()
                .ok()
                .and_then(|mut active| {
                    let current = active
                        .as_ref()
                        .is_some_and(|playback| playback.generation == generation);
                    current.then(|| active.take()).flatten()
                })
                .is_some();
            if is_current && !cancelled {
                let _ = gui_tx.send(crate::gui::GuiEvent::VoicePlaybackFinished {
                    message,
                    error: result.err(),
                });
                egui_ctx.request_repaint();
            }
        })
        .ok();
}

pub fn acknowledge_frame(generation: u64) {
    if let Ok(active) = ACTIVE.lock() {
        if let Some(playback) = active
            .as_ref()
            .filter(|playback| playback.generation == generation)
        {
            playback.frame_pending.store(false, Ordering::Release);
        }
    }
}

pub fn start(
    path: PathBuf,
    generation: u64,
    start_at_seconds: f32,
    volume: f32,
    gui_tx: crate::gui::GuiTx,
    egui_ctx: egui::Context,
) {
    let stop = Arc::new(AtomicBool::new(false));
    let frame_pending = Arc::new(AtomicBool::new(false));
    let volume_bits = Arc::new(AtomicU32::new(volume.clamp(0.0, 1.0).to_bits()));
    if let Ok(mut active) = ACTIVE.lock() {
        if let Some(previous) = active.replace(ActivePlayback {
            generation,
            stop: stop.clone(),
            frame_pending: frame_pending.clone(),
            volume_bits: volume_bits.clone(),
        }) {
            previous.stop.store(true, Ordering::Release);
        }
    }

    thread::Builder::new()
        .name("zalo-video-player".into())
        .spawn(move || {
            let duration_seconds = probe_duration_seconds(&path);
            let _ = gui_tx.send(crate::gui::GuiEvent::VideoPlaybackStarted {
                generation,
                duration_seconds,
            });
            let audio_stop = stop.clone();
            let audio_path = path.clone();
            let audio = thread::Builder::new()
                .name("zalo-video-audio".into())
                .spawn(move || {
                    let _ = play_audio(audio_path, audio_stop, start_at_seconds, volume_bits);
                });

            let result = decode_video_frames(
                &path,
                generation,
                start_at_seconds,
                &stop,
                &frame_pending,
                &gui_tx,
                &egui_ctx,
            );
            let cancelled = stop.load(Ordering::Acquire);
            stop.store(true, Ordering::Release);
            if let Ok(handle) = audio {
                let _ = handle.join();
            }
            match result {
                Ok(()) if !cancelled && !stop_was_replaced(generation) => {
                    let _ = gui_tx.send(crate::gui::GuiEvent::VideoFinished { generation });
                }
                Err(error) if !cancelled => {
                    let _ = gui_tx.send(crate::gui::GuiEvent::VideoFailed { generation, error });
                }
                _ => {}
            }
            egui_ctx.request_repaint();
        })
        .ok();
}

fn probe_duration_seconds(path: &Path) -> Option<f32> {
    let output = crate::video_tools::ffmpeg_command()
        .args(["-nostdin", "-hide_banner", "-i"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    parse_ffmpeg_duration(&String::from_utf8_lossy(&output.stderr))
}

fn parse_ffmpeg_duration(stderr: &str) -> Option<f32> {
    let value = stderr.split("Duration:").nth(1)?.trim_start();
    let clock = value.split(',').next()?.trim();
    let mut parts = clock.split(':');
    let hours = parts.next()?.parse::<f32>().ok()?;
    let minutes = parts.next()?.parse::<f32>().ok()?;
    let seconds = parts.next()?.parse::<f32>().ok()?;
    let duration = hours * 3600.0 + minutes * 60.0 + seconds;
    duration.is_finite().then_some(duration)
}

fn stop_was_replaced(generation: u64) -> bool {
    ACTIVE
        .lock()
        .ok()
        .and_then(|active| active.as_ref().map(|p| p.generation != generation))
        .unwrap_or(true)
}

fn decode_video_frames(
    path: &Path,
    generation: u64,
    start_at_seconds: f32,
    stop: &AtomicBool,
    frame_pending: &AtomicBool,
    gui_tx: &crate::gui::GuiTx,
    egui_ctx: &egui::Context,
) -> Result<(), String> {
    let filter = format!(
        "fps=30,scale={VIDEO_WIDTH}:{VIDEO_HEIGHT}:force_original_aspect_ratio=decrease,pad={VIDEO_WIDTH}:{VIDEO_HEIGHT}:(ow-iw)/2:(oh-ih)/2:color=black,format=rgba"
    );
    let mut command = crate::video_tools::ffmpeg_command();
    command.args(["-nostdin", "-re", "-loglevel", "error"]);
    if start_at_seconds > 0.0 {
        command.args(["-ss", &format!("{start_at_seconds:.3}")]);
    }
    command
        .arg("-i")
        .arg(path)
        .args([
            "-an", "-vf", &filter, "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "Không khởi động được bộ giải mã video.".to_string())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Không đọc được hình ảnh video.".to_string())?;
    let mut raw = vec![0u8; FRAME_BYTES];
    while !stop.load(Ordering::Acquire) {
        match stdout.read_exact(&mut raw) {
            Ok(()) => {
                if frame_pending
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    let image =
                        egui::ColorImage::from_rgba_unmultiplied([VIDEO_WIDTH, VIDEO_HEIGHT], &raw);
                    if gui_tx
                        .send(crate::gui::GuiEvent::VideoFrame { generation, image })
                        .is_err()
                    {
                        break;
                    }
                    egui_ctx.request_repaint();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Luồng video bị gián đoạn khi giải mã.".into());
            }
        }
    }
    let _ = child.kill();
    let status = child.wait().ok();
    if stop.load(Ordering::Acquire) || status.is_some_and(|status| status.success()) {
        Ok(())
    } else {
        Err("FFmpeg không giải mã được video này.".into())
    }
}

fn play_audio(
    path: PathBuf,
    stop: Arc<AtomicBool>,
    start_at_seconds: f32,
    volume_bits: Arc<AtomicU32>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "Không tìm thấy thiết bị phát âm thanh.".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|_| "Không đọc được cấu hình loa mặc định.".to_string())?;
    let sample_rate = config.sample_rate().0.max(8_000);
    let channels = usize::from(config.channels()).max(1);
    let ring: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(
        sample_rate as usize * channels,
    )));
    let stream_config: cpal::StreamConfig = config.clone().into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_output_f32(&device, &stream_config, ring.clone(), volume_bits.clone())
        }
        cpal::SampleFormat::I16 => {
            build_output_i16(&device, &stream_config, ring.clone(), volume_bits.clone())
        }
        cpal::SampleFormat::U16 => {
            build_output_u16(&device, &stream_config, ring.clone(), volume_bits.clone())
        }
        _ => return Err("Định dạng loa mặc định chưa được hỗ trợ.".into()),
    };
    let stream = stream.map_err(|_| "Không mở được loa mặc định.".to_string())?;
    stream
        .play()
        .map_err(|_| "Không bắt đầu phát được âm thanh.".to_string())?;

    let mut command = crate::video_tools::ffmpeg_command();
    command.args(["-nostdin", "-re", "-loglevel", "error"]);
    if start_at_seconds > 0.0 {
        command.args(["-ss", &format!("{start_at_seconds:.3}")]);
    }
    command
        .arg("-i")
        .arg(path)
        .args([
            "-vn",
            "-f",
            "f32le",
            "-ar",
            &sample_rate.to_string(),
            "-ac",
            &channels.to_string(),
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "Không khởi động được bộ giải mã âm thanh.".to_string())?;
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Không đọc được âm thanh đã giải mã.".into());
    };
    let capacity = sample_rate as usize * channels * 2;
    let mut bytes = vec![0u8; 16 * 1024];
    let mut pending_bytes = Vec::with_capacity(bytes.len() + 3);
    while !stop.load(Ordering::Acquire) {
        let Ok(read) = stdout.read(&mut bytes) else {
            break;
        };
        if read == 0 {
            break;
        }
        pending_bytes.extend_from_slice(&bytes[..read]);
        if let Ok(mut queue) = ring.lock() {
            let complete = pending_bytes.len() - (pending_bytes.len() % 4);
            let mut offset = 0;
            while offset < complete {
                queue.push_back(f32::from_le_bytes([
                    pending_bytes[offset],
                    pending_bytes[offset + 1],
                    pending_bytes[offset + 2],
                    pending_bytes[offset + 3],
                ]));
                offset += 4;
            }
            pending_bytes.drain(..complete);
            while queue.len() > capacity {
                queue.pop_front();
            }
        }
    }
    let _ = child.kill();
    let status = child.wait().ok();
    let drain_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !stop.load(Ordering::Acquire) && std::time::Instant::now() < drain_deadline {
        if ring.lock().map_or(true, |queue| queue.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    drop(stream);
    if stop.load(Ordering::Acquire) || status.is_some_and(|status| status.success()) {
        Ok(())
    } else {
        Err("FFmpeg không giải mã được tin nhắn thoại này.".into())
    }
}

fn build_output_f32(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: Arc<Mutex<VecDeque<f32>>>,
    volume_bits: Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |data: &mut [f32], _| {
            let volume = f32::from_bits(volume_bits.load(Ordering::Acquire));
            fill_output(data, &ring, |sample| sample * volume)
        },
        |_| {},
        None,
    )
}

fn build_output_i16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: Arc<Mutex<VecDeque<f32>>>,
    volume_bits: Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |data: &mut [i16], _| {
            let volume = f32::from_bits(volume_bits.load(Ordering::Acquire));
            fill_output(data, &ring, |sample| {
                ((sample * volume).clamp(-1.0, 1.0) * 32767.0) as i16
            })
        },
        |_| {},
        None,
    )
}

fn build_output_u16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: Arc<Mutex<VecDeque<f32>>>,
    volume_bits: Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |data: &mut [u16], _| {
            let volume = f32::from_bits(volume_bits.load(Ordering::Acquire));
            fill_output(data, &ring, |sample| {
                (((sample * volume).clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16
            })
        },
        |_| {},
        None,
    )
}

fn fill_output<T: Copy>(output: &mut [T], ring: &Mutex<VecDeque<f32>>, convert: impl Fn(f32) -> T) {
    if let Ok(mut queue) = ring.lock() {
        for sample in output {
            *sample = convert(queue.pop_front().unwrap_or(0.0));
        }
    } else {
        for sample in output {
            *sample = convert(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_frame_budget_is_one_fixed_1080p_or_smaller_frame() {
        assert_eq!(FRAME_BYTES, 960 * 540 * 4);
    }

    #[test]
    fn ffmpeg_duration_parser_accepts_standard_probe_output() {
        let output = "Input #0\n  Duration: 01:02:03.50, start: 0.000000, bitrate: 123 kb/s";
        assert_eq!(parse_ffmpeg_duration(output), Some(3723.5));
        assert_eq!(parse_ffmpeg_duration("Duration: N/A"), None);
    }
}
