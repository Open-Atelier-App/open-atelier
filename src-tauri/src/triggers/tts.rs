use std::process::Command;
use mp3lame_encoder::{Bitrate, Builder, DualPcm, FlushNoGap, MonoPcm, Quality};

/// Minimal RIFF/WAVE reader — just enough to read the 16-bit PCM output of
/// the OS text-to-speech commands we shell out to below. Not a general WAV
/// parser (no float/8-bit/ADPCM support): we control both ends of this
/// pipe, so there's no reason to handle formats neither `say` nor
/// `espeak-ng` actually produce.
#[derive(Debug)]
struct WavPcm {
    sample_rate: u32,
    channels: u16,
    samples: Vec<i16>,
}

fn parse_wav(bytes: &[u8]) -> Result<WavPcm, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Synthesized audio is not a valid WAV file".into());
    }

    let mut pos = 12;
    let mut sample_rate = None;
    let mut channels = None;
    let mut bits_per_sample = None;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body_start = pos + 8;
        let body_end = (body_start + chunk_size).min(bytes.len());

        if chunk_id == b"fmt " {
            if body_end < body_start + 16 {
                return Err("Malformed WAV fmt chunk".into());
            }
            channels = Some(u16::from_le_bytes(bytes[body_start + 2..body_start + 4].try_into().unwrap()));
            sample_rate = Some(u32::from_le_bytes(bytes[body_start + 4..body_start + 8].try_into().unwrap()));
            bits_per_sample = Some(u16::from_le_bytes(bytes[body_start + 14..body_start + 16].try_into().unwrap()));
        } else if chunk_id == b"data" {
            data = Some(&bytes[body_start..body_end]);
        }

        // Chunks are word-aligned: an odd-sized chunk has one padding byte.
        pos = body_end + (chunk_size % 2);
    }

    let sample_rate = sample_rate.ok_or("WAV file has no fmt chunk")?;
    let channels = channels.filter(|c| *c > 0).ok_or("WAV file reports zero channels")?;
    let bits_per_sample = bits_per_sample.ok_or("WAV file has no fmt chunk")?;
    let data = data.ok_or("WAV file has no data chunk")?;

    if bits_per_sample != 16 {
        return Err(format!("Unsupported WAV bit depth: {bits_per_sample} (expected 16-bit PCM)"));
    }

    let samples = data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    Ok(WavPcm { sample_rate, channels, samples })
}

/// Encodes 16-bit PCM WAV bytes to a real MP3 via a vendored, statically
/// linked build of libmp3lame (see mp3lame-sys) — no system lame/ffmpeg
/// install required on the user's machine.
fn encode_wav_to_mp3(wav_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let wav = parse_wav(wav_bytes)?;
    if wav.samples.is_empty() {
        return Err("Synthesized audio contains no samples".into());
    }

    let stereo = wav.channels >= 2;
    let mut builder = Builder::new().ok_or("Failed to initialize MP3 encoder")?;
    builder.set_num_channels(if stereo { 2 } else { 1 }).map_err(|e| e.to_string())?;
    builder.set_sample_rate(wav.sample_rate).map_err(|e| e.to_string())?;
    builder.set_brate(Bitrate::Kbps128).map_err(|e| e.to_string())?;
    builder.set_quality(Quality::Good).map_err(|e| e.to_string())?;
    let mut encoder = builder.build().map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    if stereo {
        let frames = wav.samples.len() / 2;
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for pair in wav.samples.chunks_exact(2) {
            left.push(pair[0]);
            right.push(pair[1]);
        }
        out.reserve(mp3lame_encoder::max_required_buffer_size(frames));
        encoder.encode_to_vec(DualPcm { left: &left, right: &right }, &mut out).map_err(|e| e.to_string())?;
    } else {
        out.reserve(mp3lame_encoder::max_required_buffer_size(wav.samples.len()));
        encoder.encode_to_vec(MonoPcm(&wav.samples), &mut out).map_err(|e| e.to_string())?;
    }

    out.reserve(7200);
    encoder.flush_to_vec::<FlushNoGap>(&mut out).map_err(|e| e.to_string())?;

    Ok(out)
}

fn engine_missing_error() -> String {
    if cfg!(target_os = "macos") {
        "Text-to-speech requires the built-in macOS `say` command, which could not be found on \
         PATH."
            .to_string()
    } else {
        "Text-to-speech requires espeak-ng (or espeak) to be installed. Install it with \
         `sudo apt install espeak-ng` (Debian/Ubuntu), `sudo pacman -S espeak-ng` (Arch), or \
         your distribution's equivalent."
            .to_string()
    }
}

/// Shells out to a local, offline OS text-to-speech engine to synthesize
/// `text` into a 16-bit PCM WAV file. macOS ships `say` out of the box;
/// Linux needs espeak-ng (or espeak) installed separately, since there's no
/// universal built-in equivalent. Nothing here ever leaves the machine —
/// this stays in line with Atelier's local-first, no-network-calls-with-
/// user-data rule the same way the rest of the trigger system does.
fn synthesize_wav(text: &str) -> Result<Vec<u8>, String> {
    let dir = std::env::temp_dir().join(format!("atelier_tts_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create temp directory: {e}"))?;
    let text_path = dir.join("input.txt");
    let wav_path = dir.join("output.wav");

    let cleanup_and_return = |dir: &std::path::Path, result: Result<Vec<u8>, String>| {
        let _ = std::fs::remove_dir_all(dir);
        result
    };

    if let Err(e) = std::fs::write(&text_path, text) {
        return cleanup_and_return(&dir, Err(format!("Cannot write temp input: {e}")));
    }

    let spawn_result = if cfg!(target_os = "macos") {
        Command::new("say")
            .arg("-o").arg(&wav_path)
            .arg("--data-format=LEI16@22050")
            .arg("-f").arg(&text_path)
            .output()
    } else {
        Command::new("espeak-ng")
            .arg("-w").arg(&wav_path)
            .arg("-f").arg(&text_path)
            .output()
            .or_else(|_| {
                Command::new("espeak")
                    .arg("-w").arg(&wav_path)
                    .arg("-f").arg(&text_path)
                    .output()
            })
    };

    let output = match spawn_result {
        Ok(o) => o,
        Err(_) => return cleanup_and_return(&dir, Err(engine_missing_error())),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return cleanup_and_return(&dir, Err(format!("Text-to-speech engine failed: {stderr}")));
    }

    let bytes = std::fs::read(&wav_path).map_err(|e| format!("Cannot read synthesized audio: {e}"));
    cleanup_and_return(&dir, bytes)
}

/// Entry point for the CREATE_MP3 trigger: local, offline text-to-speech
/// synthesis followed by MP3 encoding. Matches the `Fn(&str) ->
/// Result<Vec<u8>, String>` shape `exec_create_office` already expects, so
/// it slots in next to CREATE_DOCX/XLSX/PPTX without a bespoke executor.
pub fn text_to_mp3(text: &str) -> Result<Vec<u8>, String> {
    if text.trim().is_empty() {
        return Err("No text provided to synthesize".into());
    }
    let wav = synthesize_wav(text)?;
    encode_wav_to_mp3(&wav)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A short synthetic 16-bit PCM mono WAV (a few cycles of a sine-ish
    /// wave) — lets us test the encoder without depending on an OS TTS
    /// engine being installed at all.
    fn synthetic_wav(sample_rate: u32, channels: u16) -> Vec<u8> {
        let num_frames = 4410; // 0.1s at 44100 sample rate as a nominal length
        let mut samples: Vec<i16> = Vec::with_capacity(num_frames * channels as usize);
        for i in 0..num_frames {
            let t = i as f32 / sample_rate as f32;
            let value = (t * 440.0 * std::f32::consts::TAU).sin() * i16::MAX as f32 * 0.2;
            for _ in 0..channels {
                samples.push(value as i16);
            }
        }
        let data_bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();

        let byte_rate = sample_rate * channels as u32 * 2;
        let block_align = channels * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_bytes.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data_bytes);
        wav
    }

    #[test]
    fn parse_wav_reads_mono_pcm() {
        let wav = parse_wav(&synthetic_wav(22050, 1)).unwrap();
        assert_eq!(wav.sample_rate, 22050);
        assert_eq!(wav.channels, 1);
        assert_eq!(wav.samples.len(), 4410);
    }

    #[test]
    fn parse_wav_reads_stereo_pcm() {
        let wav = parse_wav(&synthetic_wav(44100, 2)).unwrap();
        assert_eq!(wav.channels, 2);
        assert_eq!(wav.samples.len(), 4410 * 2);
    }

    #[test]
    fn parse_wav_rejects_non_wav_bytes() {
        let err = parse_wav(b"not a wav file at all").unwrap_err();
        assert!(err.contains("valid WAV"));
    }

    #[test]
    fn encode_wav_to_mp3_produces_valid_mp3_frames() {
        let mp3 = encode_wav_to_mp3(&synthetic_wav(22050, 1)).unwrap();
        assert!(!mp3.is_empty());
        // An MP3 frame sync word is 11 set bits: 0xFF followed by a byte
        // with its top 3 bits set (0xE0 mask). LAME's raw output (no ID3v2
        // header requested here) starts directly with a frame.
        assert_eq!(mp3[0], 0xFF);
        assert_eq!(mp3[1] & 0xE0, 0xE0);
    }

    #[test]
    fn encode_wav_to_mp3_handles_stereo() {
        let mp3 = encode_wav_to_mp3(&synthetic_wav(44100, 2)).unwrap();
        assert!(!mp3.is_empty());
        assert_eq!(mp3[0], 0xFF);
    }

    #[test]
    fn encode_wav_to_mp3_rejects_garbage_input() {
        let err = encode_wav_to_mp3(b"definitely not audio").unwrap_err();
        assert!(err.contains("valid WAV"));
    }

    #[test]
    fn text_to_mp3_rejects_empty_text() {
        let err = text_to_mp3("   ").unwrap_err();
        assert!(err.contains("No text provided"));
    }

    /// Exercises the real OS TTS engine end to end (no mocking) when one is
    /// actually available in the environment running the test — macOS CI
    /// always has `say` built in, and a dev machine with espeak-ng
    /// installed gets real coverage here too. Where neither is available
    /// (e.g. a bare Linux CI image without espeak-ng), this only verifies
    /// that the failure is the specific, actionable message users need —
    /// not a generic panic or a silent empty file.
    #[test]
    fn text_to_mp3_produces_real_audio_or_a_clear_error() {
        match text_to_mp3("Hello from Atelier.") {
            Ok(bytes) => {
                assert!(!bytes.is_empty());
                assert_eq!(bytes[0], 0xFF);
                assert_eq!(bytes[1] & 0xE0, 0xE0);
            }
            Err(e) => {
                assert!(
                    e.contains("Text-to-speech requires"),
                    "unexpected TTS failure, wanted the missing-engine message: {e}"
                );
            }
        }
    }
}
