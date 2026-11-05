//! Audio and video support.
//!
//! * WAV is decoded natively (hound). Other audio formats (MP3, OGG, FLAC, AAC…)
//!   and all video formats are converted through an external `ffmpeg` binary when
//!   one is installed; FFmpeg is never linked into the application.
//! * Timeline sounds are mixed to a PCM buffer for playback and export.
//! * Playback uses the platform's command-line audio player so the application
//!   builds without native audio development headers.

use animara_document::{Document, Id, SoundAsset, SoundSync, Timeline, fresh_id};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),
    #[error("ffmpeg is required for this format but was not found on PATH")]
    NoFfmpeg,
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("{0}")]
    Other(String),
}

/// Interleaved floating point PCM.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn duration(&self) -> f64 {
        self.samples.len() as f64 / (self.sample_rate as f64 * self.channels.max(1) as f64)
    }
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
    /// Sample of channel `ch` at frame `i` (mono sources feed both channels).
    pub fn at(&self, i: usize, ch: u16) -> f32 {
        let c = self.channels.max(1);
        let ch = ch.min(c - 1);
        self.samples.get(i * c as usize + ch as usize).copied().unwrap_or(0.0)
    }
}

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn decode_wav_bytes(bytes: &[u8]) -> Result<Pcm, MediaError> {
    let mut r = hound::WavReader::new(Cursor::new(bytes))?;
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 / max)).collect::<Result<_, _>>()?
        }
    };
    Ok(Pcm { sample_rate: spec.sample_rate, channels: spec.channels, samples })
}

/// Encode PCM as 16-bit WAV.
pub fn encode_wav(pcm: &Pcm) -> Vec<u8> {
    let spec = hound::WavSpec { channels: pcm.channels.max(1), sample_rate: pcm.sample_rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cur = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cur, spec).expect("valid wav spec");
        for s in &pcm.samples {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let _ = w.finalize();
    }
    cur.into_inner()
}

fn temp_path(ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("animara-{}-{}.{}", std::process::id(), fresh_id().0, ext))
}

/// Convert any audio file to WAV bytes with ffmpeg.
pub fn ffmpeg_to_wav(path: &Path) -> Result<Vec<u8>, MediaError> {
    if !ffmpeg_available() {
        return Err(MediaError::NoFfmpeg);
    }
    let out = temp_path("wav");
    let o = Command::new("ffmpeg").args(["-v", "error", "-y", "-i"]).arg(path).args(["-vn", "-acodec", "pcm_s16le"]).arg(&out).output()?;
    if !o.status.success() {
        return Err(MediaError::Ffmpeg(String::from_utf8_lossy(&o.stderr).into_owned()));
    }
    let bytes = std::fs::read(&out)?;
    let _ = std::fs::remove_file(&out);
    Ok(bytes)
}

/// Import an audio file as a library sound (stored as WAV).
pub fn import_sound(path: &Path) -> Result<SoundAsset, MediaError> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let bytes = if ext == "wav" { std::fs::read(path)? } else { ffmpeg_to_wav(path)? };
    sound_from_wav_bytes(&path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "sound".into()), bytes)
}

pub fn sound_from_wav_bytes(name: &str, bytes: Vec<u8>) -> Result<SoundAsset, MediaError> {
    let pcm = decode_wav_bytes(&bytes)?;
    Ok(SoundAsset {
        id: fresh_id(),
        name: name.to_string(),
        format: "wav".into(),
        sample_rate: pcm.sample_rate,
        channels: pcm.channels,
        duration: pcm.duration(),
        folder: String::new(),
        data: Arc::new(bytes),
    })
}

/// Decode a library sound to PCM.
pub fn decode_sound(s: &SoundAsset) -> Result<Pcm, MediaError> {
    if s.format.eq_ignore_ascii_case("wav") {
        return decode_wav_bytes(&s.data);
    }
    let p = temp_path(&s.format);
    std::fs::write(&p, s.data.as_slice())?;
    let r = ffmpeg_to_wav(&p).and_then(|b| decode_wav_bytes(&b));
    let _ = std::fs::remove_file(&p);
    r
}

/// Peak amplitude per bucket (`n` buckets across the whole sound).
pub fn peaks(pcm: &Pcm, n: usize) -> Vec<f32> {
    let frames = pcm.frames();
    if n == 0 || frames == 0 {
        return vec![0.0; n];
    }
    (0..n)
        .map(|b| {
            let a = b * frames / n;
            let e = ((b + 1) * frames / n).max(a + 1).min(frames);
            (a..e).map(|i| (0..pcm.channels).map(|c| pcm.at(i, c).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max)
        })
        .collect()
}

/// Peaks at 100 buckets per second (used for timeline waveforms).
pub fn peaks_per_centisecond(pcm: &Pcm) -> Vec<f32> {
    peaks(pcm, (pcm.duration() * 100.0).ceil() as usize)
}

fn resample_into(src: &Pcm, rate: u32, out: &mut [f32], start_frame: i64, gain: f32, trim_start: f64, max_len: Option<f64>, loops: u32, fade_in: f64, fade_out: f64) {
    let step = src.sample_rate as f64 / rate as f64;
    let src_frames = src.frames() as f64;
    let trim0 = (trim_start * src.sample_rate as f64).max(0.0);
    let play_len = (src_frames - trim0).max(0.0);
    if play_len <= 0.0 {
        return;
    }
    let total_src = play_len * (loops as f64 + 1.0);
    let mut n_out = (total_src / step) as i64;
    if let Some(m) = max_len {
        n_out = n_out.min((m * rate as f64) as i64);
    }
    let out_frames = (out.len() / 2) as i64;
    let total_secs = n_out as f64 / rate as f64;
    for i in 0..n_out {
        let o = start_frame + i;
        if o < 0 {
            continue;
        }
        if o >= out_frames {
            break;
        }
        let pos = (i as f64 * step) % play_len + trim0;
        let i0 = pos.floor() as usize;
        let fr = (pos - pos.floor()) as f32;
        let t = i as f64 / rate as f64;
        let mut g = gain;
        if fade_in > 0.0 && t < fade_in {
            g *= (t / fade_in) as f32;
        }
        if fade_out > 0.0 && total_secs - t < fade_out {
            g *= ((total_secs - t) / fade_out).max(0.0) as f32;
        }
        for c in 0..2u16 {
            let a = src.at(i0, c);
            let b = src.at(i0 + 1, c);
            out[o as usize * 2 + c as usize] += (a + (b - a) * fr) * g;
        }
    }
}

/// Mix every sound placed on the timeline (frames `[first, last]`) into stereo PCM.
pub fn mixdown_timeline(doc: &Document, tl: &Timeline, first: u32, last: u32, rate: u32) -> Pcm {
    let fps = doc.fps.max(0.01);
    let frames = (last.saturating_sub(first) + 1) as f64;
    let len = ((frames / fps) * rate as f64).ceil() as usize;
    let mut out = vec![0.0f32; len * 2];
    let mut cache: std::collections::HashMap<Id, Pcm> = std::collections::HashMap::new();
    for layer in &tl.layers {
        if !layer.visible && layer.kind == animara_document::LayerKind::Guide {
            continue;
        }
        for (ki, k) in layer.frames.iter().enumerate() {
            let Some(s) = &k.sound else { continue };
            if s.sync == SoundSync::Stop {
                continue;
            }
            let Some(asset) = doc.library.sound(s.asset) else { continue };
            if !cache.contains_key(&s.asset) {
                match decode_sound(asset) {
                    Ok(p) => {
                        cache.insert(s.asset, p);
                    }
                    Err(_) => continue,
                }
            }
            let pcm = &cache[&s.asset];
            let start_sec = (k.start as f64 - first as f64) / fps;
            let start_frame = (start_sec * rate as f64).round() as i64;
            // Stream sounds are bound to their keyframe span; event sounds play to completion
            // (until a later Stop keyframe for the same sound).
            let max_len = match s.sync {
                SoundSync::Stream => Some(k.duration as f64 / fps),
                _ => layer.frames[ki + 1..]
                    .iter()
                    .find(|n| n.sound.as_ref().is_some_and(|x| x.asset == s.asset && x.sync == SoundSync::Stop))
                    .map(|n| (n.start - k.start) as f64 / fps),
            };
            let trimmed_len = if s.trim_end > 0.0 { Some((s.trim_end - s.trim_start).max(0.0)) } else { None };
            let max_len = match (max_len, trimmed_len) {
                (Some(a), Some(b)) => Some(a.min(b * (s.loops as f64 + 1.0))),
                (a, b) => a.or(b.map(|b| b * (s.loops as f64 + 1.0))),
            };
            resample_into(pcm, rate, &mut out, start_frame, s.volume, s.trim_start, max_len, s.loops, s.fade_in, s.fade_out);
        }
    }
    for v in &mut out {
        *v = v.clamp(-1.0, 1.0);
    }
    Pcm { sample_rate: rate, channels: 2, samples: out }
}

/// Mix all scenes back to back.
pub fn mixdown_document(doc: &Document, rate: u32) -> Pcm {
    let mut all = Pcm { sample_rate: rate, channels: 2, samples: Vec::new() };
    for s in &doc.scenes {
        let len = s.timeline.length();
        let p = mixdown_timeline(doc, &s.timeline, 0, len.saturating_sub(1), rate);
        all.samples.extend(p.samples);
    }
    all
}

/// Does the document contain any sound on a timeline?
pub fn has_audio(doc: &Document) -> bool {
    doc.scenes.iter().any(|s| s.timeline.layers.iter().any(|l| l.frames.iter().any(|k| k.sound.is_some())))
}

/// Plays PCM through the platform's command-line audio player.
#[derive(Default)]
pub struct AudioOut {
    child: Option<Child>,
    file: Option<PathBuf>,
}

impl AudioOut {
    fn player_commands(path: &Path) -> Vec<Command> {
        let mut v = Vec::new();
        if cfg!(target_os = "macos") {
            let mut c = Command::new("afplay");
            c.arg(path);
            v.push(c);
        } else if cfg!(target_os = "windows") {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-Command", &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", path.display())]);
            v.push(c);
        } else {
            for p in ["pw-play", "paplay", "aplay"] {
                let mut c = Command::new(p);
                c.arg(path);
                v.push(c);
            }
        }
        let mut c = Command::new("ffplay");
        c.args(["-nodisp", "-autoexit", "-loglevel", "quiet"]).arg(path);
        v.push(c);
        v
    }

    /// Start playing (stopping anything already playing). Returns false if no player is available.
    pub fn play(&mut self, pcm: &Pcm) -> bool {
        self.stop();
        if pcm.samples.iter().all(|s| *s == 0.0) {
            return true;
        }
        let path = temp_path("wav");
        if std::fs::write(&path, encode_wav(pcm)).is_err() {
            return false;
        }
        for mut c in Self::player_commands(&path) {
            if let Ok(child) = c.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                self.child = Some(child);
                self.file = Some(path);
                return true;
            }
        }
        let _ = std::fs::remove_file(&path);
        false
    }

    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(f) = self.file.take() {
            let _ = std::fs::remove_file(f);
        }
    }

    pub fn is_playing(&mut self) -> bool {
        match &mut self.child {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }
}

impl Drop for AudioOut {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Frames decoded from a video file for embedding on the timeline.
pub struct VideoFrames {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Vec<u8>>,
    pub audio_wav: Option<Vec<u8>>,
}

/// Decode a video into PNG frames at `fps` (max `max_frames`) plus its audio track, via ffmpeg.
pub fn import_video(path: &Path, fps: f64, max_frames: u32, max_width: u32) -> Result<VideoFrames, MediaError> {
    if !ffmpeg_available() {
        return Err(MediaError::NoFfmpeg);
    }
    let dir = temp_path("frames");
    std::fs::create_dir_all(&dir)?;
    let vf = format!("fps={fps},scale='min({max_width},iw)':-2");
    let o = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(path)
        .args(["-vf", &vf, "-frames:v", &max_frames.to_string()])
        .arg(dir.join("f%06d.png"))
        .output()?;
    if !o.status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(MediaError::Ffmpeg(String::from_utf8_lossy(&o.stderr).into_owned()));
    }
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).collect();
    names.sort();
    let mut frames = Vec::new();
    for n in &names {
        frames.push(std::fs::read(n)?);
    }
    let _ = std::fs::remove_dir_all(&dir);
    let (width, height) = frames.first().map(|f| png_size(f)).unwrap_or((0, 0));
    let audio_wav = ffmpeg_to_wav(path).ok().filter(|b| b.len() > 64);
    Ok(VideoFrames { width, height, frames, audio_wav })
}

fn png_size(b: &[u8]) -> (u32, u32) {
    if b.len() > 24 && &b[1..4] == b"PNG" {
        (u32::from_be_bytes([b[16], b[17], b[18], b[19]]), u32::from_be_bytes([b[20], b[21], b[22], b[23]]))
    } else {
        (0, 0)
    }
}

/// Simple amplitude-based lip-sync assistance: map audio loudness per frame to
/// one of `n_shapes` mouth poses (0 = closed … n-1 = widest).
pub fn lip_sync_frames(pcm: &Pcm, fps: f64, n_shapes: usize) -> Vec<usize> {
    let frames = (pcm.duration() * fps).ceil() as usize;
    let per = (pcm.sample_rate as f64 / fps).max(1.0) as usize;
    let rms: Vec<f32> = (0..frames)
        .map(|f| {
            let a = f * per;
            let e = (a + per).min(pcm.frames());
            if e <= a {
                return 0.0;
            }
            let s: f32 = (a..e).map(|i| pcm.at(i, 0).powi(2)).sum();
            (s / (e - a) as f32).sqrt()
        })
        .collect();
    let max = rms.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    rms.iter()
        .map(|r| {
            let v = r / max;
            if v < 0.08 { 0 } else { ((v * (n_shapes - 1) as f32).round() as usize).clamp(1, n_shapes - 1) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use animara_document::{Keyframe, SoundRef};

    fn tone(secs: f64) -> Vec<u8> {
        let rate = 8000;
        let n = (secs * rate as f64) as usize;
        let pcm = Pcm { sample_rate: rate, channels: 1, samples: (0..n).map(|i| (i as f32 * 0.3).sin() * 0.5).collect() };
        encode_wav(&pcm)
    }

    #[test]
    fn wav_roundtrip_and_peaks() {
        let s = sound_from_wav_bytes("t.wav", tone(1.0)).unwrap();
        assert!((s.duration - 1.0).abs() < 0.01);
        let pcm = decode_sound(&s).unwrap();
        let p = peaks(&pcm, 10);
        assert!(p.iter().all(|v| *v > 0.4 && *v <= 0.51));
    }

    #[test]
    fn stream_sound_is_bounded_by_span() {
        let mut doc = Document::new(100.0, 100.0, 10.0);
        let s = sound_from_wav_bytes("t.wav", tone(2.0)).unwrap();
        let id = s.id;
        doc.library.sounds.push(s);
        let layer = &mut doc.scenes[0].timeline.layers[0];
        layer.frames = vec![Keyframe::new(0, 5), Keyframe::new(5, 15)];
        layer.frames[0].sound = Some(SoundRef::new(id));
        let pcm = mixdown_timeline(&doc, &doc.scenes[0].timeline, 0, 19, 8000);
        assert!((pcm.duration() - 2.0).abs() < 0.01);
        let at = |t: f64| pcm.samples[(t * 8000.0) as usize * 2].abs();
        let loud: f32 = (0..100).map(|i| at(0.1 + i as f64 * 0.003)).sum();
        let quiet: f32 = (0..100).map(|i| at(0.8 + i as f64 * 0.003)).sum();
        assert!(loud > 1.0 && quiet == 0.0, "{loud} {quiet}");
    }

    #[test]
    fn lip_sync_detects_silence() {
        let mut samples = vec![0.0f32; 8000];
        samples.extend((0..8000).map(|i| (i as f32 * 0.3).sin() * 0.8));
        let pcm = Pcm { sample_rate: 8000, channels: 1, samples };
        let f = lip_sync_frames(&pcm, 10.0, 4);
        assert_eq!(f[0], 0);
        assert!(f[15] > 0);
    }
}
