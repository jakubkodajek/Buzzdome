//! Writes a clip as a 16-bit mono WAV file, which every window plays.

use std::io::{self, Write};

/// Writes `samples` (from -1 to 1) at `rate` per second to `out`.
pub fn write(mut out: impl Write, samples: &[f32], rate: u32) -> io::Result<()> {
    let data = u32::try_from(samples.len() * 2).map_err(|_| io::Error::other("clip too long"))?;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?; // PCM
    out.write_all(&1u16.to_le_bytes())?; // mono
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&(rate * 2).to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data.to_le_bytes())?;
    let pcm: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16).to_le_bytes()).collect();
    out.write_all(&pcm)?;
    out.flush()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_clip_is_a_mono_16_bit_wav() {
        let mut out = Vec::new();
        super::write(&mut out, &[0.0, 1.0, -1.0, 2.0], 44_100).unwrap();
        assert_eq!(out.len(), 44 + 8);
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(out[24..28].try_into().unwrap()), 44_100);
        assert_eq!(&out[44..], &[0, 0, 0xff, 0x7f, 0x01, 0x80, 0xff, 0x7f]);
    }
}
