//! Recognises container and codec of an audio file from its first bytes. Used for
//! clear error messages ("Ogg Opus is not supported") instead of a generic
//! decoder error; the actual decoding happens in the app (rodio/symphonia).

use std::io::Read;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioKind {
    OggVorbis,
    OggOpus,
    OggFlac,
    OggSpeex,
    /// Ogg container with an unknown first stream.
    OggOther,
    Wav,
    Flac,
    Mp3,
    Mp4,
    Empty,
    Unknown,
}

impl AudioKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OggVorbis => "Ogg Vorbis",
            Self::OggOpus => "Ogg Opus",
            Self::OggFlac => "Ogg FLAC",
            Self::OggSpeex => "Ogg Speex",
            Self::OggOther => "Ogg (unknown codec)",
            Self::Wav => "WAV",
            Self::Flac => "FLAC",
            Self::Mp3 => "MP3",
            Self::Mp4 => "MP4/AAC",
            Self::Empty => "empty file",
            Self::Unknown => "unknown format",
        }
    }

    /// Formats the app decodes (rodio with symphonia: ogg+vorbis, wav, flac, mp3).
    pub fn supported(self) -> bool {
        matches!(self, Self::OggVorbis | Self::Wav | Self::Flac | Self::Mp3)
    }
}

/// Number of bytes [`sniff`] needs at most.
pub const SNIFF_BYTES: usize = 512;

pub fn sniff(head: &[u8]) -> AudioKind {
    if head.is_empty() {
        return AudioKind::Empty;
    }
    if head.starts_with(b"OggS") {
        // first page: 27 byte header, segment table, then the first packet
        let Some(&segments) = head.get(26) else {
            return AudioKind::OggOther;
        };
        let packet = head.get(27 + usize::from(segments)..).unwrap_or_default();
        return if packet.starts_with(b"\x01vorbis") {
            AudioKind::OggVorbis
        } else if packet.starts_with(b"OpusHead") {
            AudioKind::OggOpus
        } else if packet.starts_with(b"\x7fFLAC") {
            AudioKind::OggFlac
        } else if packet.starts_with(b"Speex   ") {
            AudioKind::OggSpeex
        } else {
            AudioKind::OggOther
        };
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE" {
        return AudioKind::Wav;
    }
    if head.starts_with(b"fLaC") {
        return AudioKind::Flac;
    }
    if head.starts_with(b"ID3") || (head.len() >= 2 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0) {
        return AudioKind::Mp3;
    }
    if head.len() >= 8 && &head[4..8] == b"ftyp" {
        return AudioKind::Mp4;
    }
    AudioKind::Unknown
}

pub fn sniff_file(path: &Path) -> std::io::Result<AudioKind> {
    let mut buf = Vec::with_capacity(SNIFF_BYTES);
    std::fs::File::open(path)?
        .take(SNIFF_BYTES as u64)
        .read_to_end(&mut buf)?;
    Ok(sniff(&buf))
}

/// The file starts like an audio file (any container [`sniff`] knows, also
/// the ones the app cannot play). Used where a pack definition names files
/// outside the pack: anything else is never copied.
pub fn is_audio_file(path: &Path) -> std::io::Result<bool> {
    Ok(!matches!(
        sniff_file(path)?,
        AudioKind::Unknown | AudioKind::Empty
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio")
            .join(name)
    }

    #[test]
    fn recognises_fixtures() {
        assert_eq!(
            sniff_file(&fixture("tone.ogg")).unwrap(),
            AudioKind::OggVorbis
        );
        assert_eq!(sniff_file(&fixture("tone.wav")).unwrap(), AudioKind::Wav);
        assert_eq!(sniff_file(&fixture("tone.flac")).unwrap(), AudioKind::Flac);
        assert_eq!(sniff_file(&fixture("tone.mp3")).unwrap(), AudioKind::Mp3);
    }

    #[test]
    fn recognises_opus_and_garbage() {
        let mut opus = b"OggS\x00\x02".to_vec();
        opus.resize(26, 0);
        opus.push(1); // one segment
        opus.push(19);
        opus.extend_from_slice(b"OpusHead\x01\x01\x38\x01\x80\xbb\x00\x00\x00\x00\x00");
        assert_eq!(sniff(&opus), AudioKind::OggOpus);
        assert!(!AudioKind::OggOpus.supported());
        assert_eq!(sniff(b""), AudioKind::Empty);
        assert_eq!(sniff(b"OggS"), AudioKind::OggOther);
        assert_eq!(sniff(b"<html>"), AudioKind::Unknown);
    }
}
