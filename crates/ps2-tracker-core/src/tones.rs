//! Self-generated tones (sine with envelope) as WAV – no third-party sounds.

/// A note: frequency in Hz (0 = rest), duration in ms.
#[derive(Debug, Clone, Copy)]
pub struct Note(pub f32, pub u32);

pub const SAMPLE_RATE: u32 = 44_100;

/// Generates 16-bit mono PCM for a sequence of notes.
pub fn synth(notes: &[Note], gain: f32) -> Vec<i16> {
    let mut out = Vec::new();
    for &Note(freq, ms) in notes {
        let n = (SAMPLE_RATE as u64 * ms as u64 / 1000) as usize;
        let attack = (SAMPLE_RATE as usize / 200).min(n / 4).max(1); // 5 ms
        let release = (SAMPLE_RATE as usize / 25).min(n / 2).max(1); // 40 ms
        for i in 0..n {
            if freq <= 0.0 {
                out.push(0);
                continue;
            }
            let t = i as f32 / SAMPLE_RATE as f32;
            let env = if i < attack {
                i as f32 / attack as f32
            } else if i + release > n {
                (n - i) as f32 / release as f32
            } else {
                1.0
            };
            // fundamental plus a quiet octave for a fuller sound
            let v = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.8
                + (4.0 * std::f32::consts::PI * freq * t).sin() * 0.2;
            out.push((v * env * gain.clamp(0.0, 1.0) * i16::MAX as f32) as i16);
        }
    }
    out
}

/// Wraps PCM samples as a WAV file (RIFF, PCM 16 bit, mono).
pub fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    b.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

/// Note sequences of the default pack per event (file name, notes).
pub fn standard_set() -> Vec<(&'static str, &'static str, Vec<Note>)> {
    use Note as N;
    vec![
        ("kill", "kill.wav", vec![N(880.0, 60)]),
        (
            "headshot",
            "headshot.wav",
            vec![N(1175.0, 50), N(1568.0, 70)],
        ),
        (
            "multikill",
            "multikill_2.wav",
            vec![N(988.0, 60), N(0.0, 30), N(1319.0, 90)],
        ),
        (
            "multikill",
            "multikill_3.wav",
            vec![N(988.0, 60), N(1175.0, 60), N(1568.0, 120)],
        ),
        (
            "multikill",
            "multikill_4.wav",
            vec![N(988.0, 50), N(1175.0, 50), N(1319.0, 50), N(1760.0, 160)],
        ),
        (
            "streak",
            "streak.wav",
            vec![N(659.0, 80), N(784.0, 80), N(988.0, 80), N(1319.0, 200)],
        ),
        (
            "streak_ended",
            "streak_ended.wav",
            vec![N(784.0, 120), N(523.0, 220)],
        ),
        ("death", "death.wav", vec![N(392.0, 120), N(262.0, 220)]),
        (
            "suicide",
            "suicide.wav",
            vec![N(330.0, 90), N(0.0, 40), N(220.0, 260)],
        ),
        (
            "teamkill",
            "teamkill.wav",
            vec![N(466.0, 100), N(466.0, 100), N(466.0, 100)],
        ),
        (
            "teamkilled",
            "teamkilled.wav",
            vec![N(311.0, 150), N(311.0, 150)],
        ),
        ("revive", "revive.wav", vec![N(523.0, 70), N(784.0, 120)]),
        ("revived", "revived.wav", vec![N(784.0, 70), N(1047.0, 140)]),
        (
            "revenge",
            "revenge.wav",
            vec![N(587.0, 80), N(880.0, 80), N(1175.0, 160)],
        ),
        (
            "nemesis",
            "nemesis.wav",
            vec![N(220.0, 160), N(233.0, 160), N(220.0, 240)],
        ),
        (
            "vehicle_kill",
            "vehicle_kill.wav",
            vec![N(196.0, 80), N(392.0, 80), N(784.0, 140)],
        ),
        (
            "achievement",
            "achievement.wav",
            vec![N(784.0, 90), N(988.0, 90), N(1175.0, 90), N(1568.0, 260)],
        ),
        (
            "login",
            "login.wav",
            vec![N(523.0, 100), N(659.0, 100), N(784.0, 180)],
        ),
        (
            "session_start",
            "session_start.wav",
            vec![N(659.0, 90), N(784.0, 160)],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let s = synth(&[Note(440.0, 100)], 0.5);
        assert_eq!(s.len(), 4410);
        let w = wav(&s);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 8820);
        assert_eq!(w.len(), 44 + 8820);
    }

    #[test]
    fn envelope_starts_and_ends_silent() {
        let s = synth(&[Note(440.0, 200)], 1.0);
        assert_eq!(s[0], 0);
        assert!(s.last().unwrap().abs() < 1000);
        assert!(s.iter().map(|x| x.unsigned_abs()).max().unwrap() > 20_000);
    }
}
