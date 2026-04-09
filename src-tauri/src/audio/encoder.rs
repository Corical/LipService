use hound::{WavSpec, WavWriter};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::io::Cursor;

const TARGET_SAMPLE_RATE: u32 = 16_000;
const TARGET_CHANNELS: u16 = 1;
const TARGET_BITS_PER_SAMPLE: u16 = 16;

#[derive(Debug, thiserror::Error)]
pub enum EncoderError {
    #[error("Audio buffer is empty")]
    EmptyBuffer,
    #[error("WAV encoding failed: {0}")]
    WavWrite(String),
    #[error("Resampling failed: {0}")]
    Resample(String),
}

/// Converts raw f32 PCM samples to a 16kHz mono WAV byte buffer.
pub fn encode_to_wav(
    samples: &[f32],
    source_sample_rate: u32,
    source_channels: u16,
) -> Result<Vec<u8>, EncoderError> {
    if samples.is_empty() {
        return Err(EncoderError::EmptyBuffer);
    }

    let mono_samples = if source_channels > 1 {
        mix_to_mono(samples, source_channels)
    } else {
        samples.to_vec()
    };

    let resampled = if source_sample_rate != TARGET_SAMPLE_RATE {
        resample(&mono_samples, source_sample_rate, TARGET_SAMPLE_RATE)?
    } else {
        mono_samples
    };

    let spec = WavSpec {
        channels: TARGET_CHANNELS,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: TARGET_BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(&mut cursor, spec)
        .map_err(|e| EncoderError::WavWrite(e.to_string()))?;

    for &sample in &resampled {
        let clamped = sample.clamp(-1.0, 1.0);
        let int_sample = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(int_sample)
            .map_err(|e| EncoderError::WavWrite(e.to_string()))?;
    }

    writer.finalize()
        .map_err(|e| EncoderError::WavWrite(e.to_string()))?;

    Ok(cursor.into_inner())
}

fn mix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn resample(
    mono_samples: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<f32>, EncoderError> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = to_rate as f64 / from_rate as f64;
    let chunk_size = 1024;

    let mut resampler = SincFixedIn::<f32>::new(
        ratio,
        2.0,
        params,
        chunk_size,
        1,
    ).map_err(|e| EncoderError::Resample(e.to_string()))?;

    let mut output = Vec::new();

    for chunk in mono_samples.chunks(chunk_size) {
        let input = if chunk.len() < chunk_size {
            let mut padded = chunk.to_vec();
            padded.resize(chunk_size, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        let result = resampler.process(&[input], None)
            .map_err(|e: rubato::ResampleError| EncoderError::Resample(e.to_string()))?;

        if let Some(channel) = result.first() {
            output.extend_from_slice(channel);
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_empty_buffer_returns_error() {
        let result = encode_to_wav(&[], 44100, 1);
        assert!(matches!(result.unwrap_err(), EncoderError::EmptyBuffer));
    }

    #[test]
    fn test_encode_mono_16khz_passthrough() {
        let samples: Vec<f32> = (0..16000)
            .map(|i| (i as f32 / 16000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5)
            .collect();
        let wav = encode_to_wav(&samples, 16000, 1).unwrap();
        assert!(wav.len() > 44);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn test_encode_stereo_mixes_to_mono() {
        let mut samples = Vec::new();
        for i in 0..8000 {
            let val = (i as f32 / 8000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            samples.push(val);
            samples.push(val);
        }
        let wav = encode_to_wav(&samples, 16000, 2).unwrap();
        assert!(wav.len() > 44);
        assert_eq!(&wav[0..4], b"RIFF");
    }

    #[test]
    fn test_encode_44100_resamples_to_16khz() {
        let samples: Vec<f32> = (0..44100)
            .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5)
            .collect();
        let wav = encode_to_wav(&samples, 44100, 1).unwrap();
        assert!(wav.len() > 44);
        assert_eq!(&wav[0..4], b"RIFF");
    }

    #[test]
    fn test_encode_clamps_out_of_range_samples() {
        let samples = vec![2.0, -2.0, 0.5, -0.5, 1.5, -1.5];
        let wav = encode_to_wav(&samples, 16000, 1).unwrap();
        assert!(wav.len() > 44);
    }
}
