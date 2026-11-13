//! Native voice capture with sample-format conversion and phase-preserving resampling.
use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
/// Private encoded recording; dropping it removes temporary audio.
pub struct VoiceRecord {
    pub path: std::path::PathBuf,
    pub mime: String,
    pub duration_secs: u64,
    pub samples: u64,
    pub dir: Option<std::path::PathBuf>,
}

impl VoiceRecord {
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(d) = self.dir.as_ref() {
            let _ = std::fs::remove_dir(d);
        }
    }
}

/// Create an owner-only temporary directory with atomic collision handling.
pub(crate) fn voice_temp_dir(tag: i64) -> Result<std::path::PathBuf> {
    let base = std::env::temp_dir();
    for attempt in 0..17u32 {
        let suffix = if attempt == 0 {
            format!("{tag}")
        } else {
            format!("{tag}-{attempt}")
        };
        let dir = base.join(format!("ctdozlite-voice-{}-{suffix}", std::process::id()));
        if let Ok(meta) = std::fs::symlink_metadata(&dir) {
            if meta.file_type().is_symlink() {
                anyhow::bail!("Thư mục tạm voice không an toàn (symlink)");
            }
            continue; // tên đã có thật → thử suffix khác
        }
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
                }
                let meta = std::fs::symlink_metadata(&dir)
                    .map_err(|e| anyhow::anyhow!("Không stat được thư mục tạm voice ({e})"))?;
                if !meta.is_dir() || meta.file_type().is_symlink() {
                    let _ = std::fs::remove_dir(&dir);
                    anyhow::bail!("Thư mục tạm voice không an toàn");
                }
                return Ok(dir);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => anyhow::bail!("Không tạo được thư mục tạm voice ({e})"),
        }
    }
    anyhow::bail!("Không tạo được thư mục tạm voice (trùng tên)")
}

const VOICE_TARGET_RATE: u32 = 16_000;
const VOICE_MAX_SAMPLES: usize = 16000 * 300;

struct RecResampler {
    ratio: f64,
    buf: Vec<f32>,
    src: f64,
}

impl RecResampler {
    fn new(in_rate: u32) -> Self {
        Self {
            ratio: in_rate.max(1) as f64 / VOICE_TARGET_RATE as f64,
            buf: Vec::new(),
            src: 0.0,
        }
    }

    fn push_frames(&mut self, frames: &[f32]) -> Vec<f32> {
        self.buf.extend_from_slice(frames);
        let len = self.buf.len();
        let mut out = Vec::with_capacity((frames.len() as f64 / self.ratio).ceil() as usize + 1);
        while self.src + 1.0 < len as f64 {
            let i0 = self.src as usize;
            let frac = (self.src - i0 as f64) as f32;
            let a = self.buf[i0];
            out.push(a + (self.buf[i0 + 1] - a) * frac);
            self.src += self.ratio;
        }
        let consumed = (self.src as usize).min(self.buf.len());
        self.buf.drain(..consumed);
        self.src -= consumed as f64;
        out
    }

    fn flush(&mut self) -> Vec<f32> {
        let mut out = Vec::new();
        let len = self.buf.len();
        while self.src < len as f64 {
            out.push(self.buf[(self.src as usize).min(len.saturating_sub(1))]);
            self.src += self.ratio;
        }
        self.buf.clear();
        self.src = 0.0;
        out
    }
}

#[cfg(test)]
pub(crate) fn resample_mono_to_16k(input: &[f32], in_rate: u32) -> Vec<f32> {
    let mut r = RecResampler::new(in_rate);
    let mut out = r.push_frames(input);
    out.extend(r.flush());
    out
}

#[derive(Clone)]
struct RecIngest {
    pcm: Arc<Mutex<Vec<f32>>>,
    resamp: Arc<Mutex<RecResampler>>,
    stop: Arc<AtomicBool>,
    last_peak: Arc<Mutex<f32>>,
}

fn ingest_interleaved<S: Copy>(st: &RecIngest, data: &[S], channels: usize, sample: fn(S) -> f32) {
    if st.stop.load(Ordering::Relaxed) {
        return;
    }
    let ch = channels.max(1);
    let total_frames = data.len() / ch;
    if total_frames == 0 {
        return;
    }
    let mut mono = Vec::with_capacity(total_frames);
    for f in 0..total_frames {
        let mut sum = 0.0f32;
        for c in 0..ch {
            sum += sample(data[f * ch + c]);
        }
        mono.push(sum / ch as f32);
    }
    let mut peak = 0.0f32;
    if let (Ok(mut resamp), Ok(mut pcm)) = (st.resamp.lock(), st.pcm.lock()) {
        for v in resamp.push_frames(&mono) {
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            pcm.push(v);
        }
        if pcm.len() > VOICE_MAX_SAMPLES {
            pcm.truncate(VOICE_MAX_SAMPLES);
            st.stop.store(true, Ordering::Relaxed);
        }
    }

    if let Ok(mut g) = st.last_peak.lock() {
        *g = peak;
    }
}

/// Microphone capture owned and stopped on one native thread.
pub struct VoiceRecorder {
    stop: Arc<AtomicBool>,
    _stream: Option<cpal::Stream>,
    pcm: Arc<Mutex<Vec<f32>>>,
    resamp: Arc<Mutex<RecResampler>>,
    last_peak: Arc<Mutex<f32>>,
    error: Arc<Mutex<Option<String>>>,
    started_at: std::time::Instant,
}

impl Drop for VoiceRecorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl VoiceRecorder {
    pub fn start() -> Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let input_dev = host.default_input_device().ok_or_else(|| {
            anyhow::anyhow!("Không tìm thấy micro trên máy. Hãy cắm micro rồi thử lại.")
        })?;
        let in_cfg = input_dev
            .default_input_config()
            .map_err(|e| mic_friendly_error("Không mở được micro", &e.to_string()))?;
        let in_rate = in_cfg.sample_rate().0.max(1);
        let in_channels = (in_cfg.channels() as usize).max(1);
        let sample_format = in_cfg.sample_format();
        let in_cfg_std: cpal::StreamConfig = in_cfg.into();
        let pcm: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(16000 * 8)));
        let resamp: Arc<Mutex<RecResampler>> = Arc::new(Mutex::new(RecResampler::new(in_rate)));
        let last_peak: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
        let stop = Arc::new(AtomicBool::new(false));
        let st = RecIngest {
            pcm: pcm.clone(),
            resamp: resamp.clone(),
            stop: stop.clone(),
            last_peak: last_peak.clone(),
        };
        let error = Arc::new(Mutex::new(None::<String>));
        let mk_err = |err_cb: Arc<Mutex<Option<String>>>| {
            move |e: cpal::StreamError| {
                if let Ok(mut g) = err_cb.lock() {
                    if g.is_none() {
                        *g = Some(
                            mic_friendly_error("Lỗi micro khi đang thu", &e.to_string())
                                .to_string(),
                        );
                    }
                }
            }
        };
        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let st = st.clone();
                input_dev.build_input_stream(
                    &in_cfg_std,
                    move |data: &[f32], _| ingest_interleaved(&st, data, in_channels, |v| v),
                    mk_err(error.clone()),
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let st = st.clone();
                input_dev.build_input_stream(
                    &in_cfg_std,
                    move |data: &[i16], _| {
                        ingest_interleaved(&st, data, in_channels, |v| v as f32 / 32768.0)
                    },
                    mk_err(error.clone()),
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let st = st.clone();
                input_dev.build_input_stream(
                    &in_cfg_std,
                    move |data: &[u16], _| {
                        ingest_interleaved(&st, data, in_channels, |v| {
                            (v as f32 - 32768.0) / 32768.0
                        })
                    },
                    mk_err(error.clone()),
                    None,
                )
            }
            fmt => {
                return Err(mic_friendly_error(
                    "Không mở được micro để thu",
                    &format!("định dạng mẫu không hỗ trợ ({fmt:?})"),
                ));
            }
        }
        .map_err(|e| mic_friendly_error("Không mở được micro để thu", &e.to_string()))?;
        stream.play().map_err(|e| {
            anyhow::anyhow!("Không bắt đầu thu được ({e}). Kiểm tra quyền Micro trong System Settings → Privacy & Security → Microphone.")
        })?;
        Ok(Self {
            stop,
            _stream: Some(stream),
            pcm,
            resamp,
            last_peak,
            error,
            started_at: std::time::Instant::now(),
        })
    }

    pub fn peak(&self) -> f32 {
        self.last_peak.lock().map(|p| *p).unwrap_or(0.0)
    }
    pub fn samples(&self) -> u64 {
        self.pcm.lock().map(|b| b.len() as u64).unwrap_or(0)
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut g| g.take())
    }

    pub fn cancel(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        drop(self._stream.take());
    }

    pub fn stop_and_encode(mut self) -> Result<VoiceRecord> {
        self.stop.store(true, Ordering::Relaxed);
        drop(self._stream.take());
        if let (Ok(mut resamp), Ok(mut pcm)) = (self.resamp.lock(), self.pcm.lock()) {
            pcm.extend(resamp.flush());
        }
        let samples_vec: Vec<f32> = self.pcm.lock().map(|b| b.clone()).unwrap_or_default();
        if samples_vec.is_empty() {
            anyhow::bail!("Chưa thu được âm thanh nào — hãy giữ micro và nói to hơn.");
        }
        let duration_secs = (samples_vec.len() as u64).div_ceil(16000).max(1);
        let tag = crate::login::unix_now();
        let samples_n = samples_vec.len() as u64;
        let (path, dir) = encode_mono_aac_16k(&samples_vec, tag)?;
        Ok(VoiceRecord {
            path,
            mime: "audio/aac".into(),
            duration_secs,
            samples: samples_n,
            dir: Some(dir),
        })
    }
}
impl Drop for VoiceRecord {
    fn drop(&mut self) {
        self.cleanup();
    }
}
fn mic_friendly_error(context: &str, _raw: &str) -> anyhow::Error {
    anyhow::anyhow!("{context}. Kiểm tra micro và quyền Microphone trong cài đặt hệ thống.")
}
struct EncoderProcess(std::process::Child);
impl Drop for EncoderProcess {
    fn drop(&mut self) {
        // Best-effort cleanup also covers early I/O failures; reaped-child errors are harmless.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// Encode bounded mono samples into AAC with the managed FFmpeg executable.
pub fn encode_mono_aac_16k(
    samples: &[f32],
    tag: i64,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    use std::io::Write;
    anyhow::ensure!(
        !samples.is_empty() && samples.len() <= VOICE_MAX_SAMPLES,
        "Thời lượng ghi âm không hợp lệ."
    );
    let dir = voice_temp_dir(tag)?;
    let result = (|| -> Result<std::path::PathBuf> {
        let path = dir.join("voice.aac");
        // Preserve owner-only permissions even if encoding is interrupted before cleanup.
        crate::data_dir::write_private_file(&path, &[])?;
        let process = crate::video_tools::ffmpeg_command()
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "f32le",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-i",
                "pipe:0",
                "-c:a",
                "aac",
                "-b:a",
                "64k",
                "-f",
                "adts",
            ])
            .arg(&path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let mut process = EncoderProcess(process);
        let child = &mut process.0;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Không mở được bộ mã hóa âm thanh."))?;
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|s| s.clamp(-1.0, 1.0).to_le_bytes())
            .collect();
        let writer = std::thread::spawn(move || stdin.write_all(&bytes));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                anyhow::bail!("Mã hóa ghi âm quá thời gian.");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("Mã hóa ghi âm bị gián đoạn."))??;
        anyhow::ensure!(
            status.success() && std::fs::metadata(&path)?.len() > 0,
            "Không mã hóa được ghi âm."
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(path)
    })();
    match result {
        Ok(path) => Ok((path, dir)),
        Err(error) => {
            let _ = std::fs::remove_dir_all(dir);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn managed_aac_roundtrip_when_tool_fixture_is_supplied() {
        let Ok(directory) = std::env::var("CTDOZLITE_TEST_FFMPEG_DIR") else {
            return;
        };
        assert!(crate::video_tools::refresh(std::path::Path::new(&directory)).await);
        let pcm = sine_1s(16000, 440.0);
        let (path, dir) = encode_mono_aac_16k(&pcm, 88).unwrap();
        let record = VoiceRecord {
            path: path.clone(),
            dir: Some(dir.clone()),
            mime: "audio/aac".into(),
            duration_secs: 1,
            samples: 16000,
        };
        let encoded = std::fs::read(&path).unwrap();
        assert_eq!(encoded[0], 0xff);
        assert_eq!(encoded[1] & 0xf0, 0xf0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let decoded = crate::video_tools::ffmpeg_command()
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args(["-f", "f32le", "-ac", "1", "-ar", "16000", "pipe:1"])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        assert!((16000 * 4..=19000 * 4).contains(&decoded.stdout.len()));
        drop(record);
        assert!(!path.exists());
        assert!(!dir.exists());
    }

    fn sine_1s(in_rate: u32, freq: f32) -> Vec<f32> {
        (0..in_rate)
            .map(|n| (2.0 * std::f32::consts::PI * freq * n as f32 / in_rate as f32).sin())
            .collect()
    }

    #[test]
    fn resample_8k_to_16k_preserves_duration() {
        let out = resample_mono_to_16k(&sine_1s(8000, 440.0), 8000);
        assert!(
            (out.len() as i64 - 16000).abs() <= 1,
            "8k 1s -> 16k mẫu, got {}",
            out.len()
        );
        let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32).sqrt();
        assert!(rms > 0.5, "sine méo sau upsample, rms={rms}");
    }

    #[test]
    fn resample_441k_to_16k_frame_count() {
        let out = resample_mono_to_16k(&sine_1s(44100, 440.0), 44100);
        assert!(
            (out.len() as i64 - 16000).abs() <= 1,
            "44.1k 1s -> 16k mẫu, got {}",
            out.len()
        );
    }

    #[test]
    fn resampler_phase_survives_callback_boundaries() {
        let input = sine_1s(44100, 440.0);
        let one_shot = resample_mono_to_16k(&input, 44100);
        let mut r = RecResampler::new(44100);
        let mut chunked = Vec::new();
        for chunk in input.chunks(128) {
            chunked.extend(r.push_frames(chunk));
        }
        chunked.extend(r.flush());
        assert!(
            (chunked.len() as i64 - one_shot.len() as i64).abs() <= 1,
            "chunked {} vs one-shot {}",
            chunked.len(),
            one_shot.len()
        );
        assert!(
            (chunked.len() as i64 - 16000).abs() <= 2,
            "44.1k 1s chunked -> 16k mẫu, got {}",
            chunked.len()
        );
    }
}
