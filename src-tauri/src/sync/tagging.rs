//! Embeds cover art (and optionally text tags) into audio files with lofty.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::items::Timestamp;
use lofty::tag::Tag;

use crate::error::{Error, Result};
use crate::paths::TrackFields;

/// Rewrites tags of `path` in place.
/// `cover`: replaces all embedded pictures with this JPEG as front cover.
/// `fields`: also writes text tags (used for transcoded files).
pub fn apply(path: &Path, cover: Option<&[u8]>, fields: Option<&TrackFields>) -> Result<()> {
    if cover.is_none() && fields.is_none() {
        return Ok(());
    }
    let mut tagged = lofty::read_from_path(path)?;
    if tagged.primary_tag().is_none() {
        let tag_type = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| Error::Other("file type has no writable tag".into()))?;

    if let Some(jpeg) = cover {
        while !tag.pictures().is_empty() {
            tag.remove_picture(0);
        }
        tag.push_picture(
            Picture::unchecked(jpeg.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
    }

    if let Some(f) = fields {
        tag.set_title(f.title.clone());
        tag.set_artist(f.artist.clone());
        tag.set_album(f.album.clone());
        tag.insert_text(ItemKey::AlbumArtist, f.albumartist.clone());
        if !f.genre.is_empty() {
            tag.set_genre(f.genre.clone());
        }
        if let Some(t) = f.track {
            tag.set_track(t);
        }
        if let Some(d) = f.disc {
            tag.set_disk(d);
        }
        if let Some(y) = f.year {
            tag.set_date(Timestamp {
                year: y.min(u16::MAX as u32) as u16,
                ..Default::default()
            });
        }
    }

    // ID3v2.3 is the most widely supported by hardware players.
    tag.save_to_path(path, WriteOptions::default().use_id3v23(true))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::file::TaggedFileExt;

    /// MPEG-1 Layer III, 128 kbps, 44.1 kHz frames of silence.
    fn mp3() -> Vec<u8> {
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        frame.repeat(20)
    }

    fn flac() -> Vec<u8> {
        let mut v = b"fLaC".to_vec();
        v.extend_from_slice(&[0x80, 0, 0, 34]); // last block, STREAMINFO, len 34
        v.extend_from_slice(&[0x10, 0x00, 0x10, 0x00]); // min/max block size 4096
        v.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // min/max frame size unknown
        // 44100 Hz (20 bits) | 2 ch - 1 (3 bits) | 16 bps - 1 (5 bits) | 36-bit total samples = 44100
        let packed: u64 = (44100u64 << 44) | (1 << 41) | (15 << 36) | 44100;
        v.extend_from_slice(&packed.to_be_bytes());
        v.extend_from_slice(&[0u8; 16]); // md5
        v
    }

    fn wav() -> Vec<u8> {
        let data = vec![0u8; 44100 * 4 / 10];
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&44100u32.to_le_bytes());
        v.extend_from_slice(&(44100u32 * 4).to_le_bytes());
        v.extend_from_slice(&4u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(&data);
        v
    }

    fn jpeg() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 10, 10]));
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut out).encode_image(&img).unwrap();
        out
    }

    fn fields() -> TrackFields {
        TrackFields {
            albumartist: "Album Artist".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            title: "Title".into(),
            genre: "Jazz".into(),
            year: Some(2001),
            track: Some(3),
            disc: Some(2),
        }
    }

    fn check(name: &str, bytes: Vec<u8>, with_fields: bool) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        let cover = jpeg();
        // Twice: second run must replace, not append, the picture.
        for _ in 0..2 {
            apply(&p, Some(&cover), with_fields.then(fields).as_ref()).unwrap();
        }
        let f = lofty::read_from_path(&p).unwrap();
        let tag = f.primary_tag().expect("tag written");
        assert_eq!(tag.pictures().len(), 1, "{name}");
        let pic = &tag.pictures()[0];
        assert_eq!(pic.pic_type(), PictureType::CoverFront);
        assert_eq!(pic.data(), cover.as_slice());
        if with_fields {
            assert_eq!(tag.title().as_deref(), Some("Title"));
            assert_eq!(tag.artist().as_deref(), Some("Artist"));
            assert_eq!(tag.track(), Some(3));
            assert_eq!(tag.get_string(ItemKey::AlbumArtist), Some("Album Artist"));
        }
    }

    #[test]
    fn mp3_cover_and_tags() {
        check("a.mp3", mp3(), true);
    }

    #[test]
    fn flac_cover_and_tags() {
        check("a.flac", flac(), true);
    }

    #[test]
    fn wav_cover() {
        check("a.wav", wav(), false);
    }

    #[test]
    fn noop_without_changes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.mp3");
        std::fs::write(&p, b"garbage").unwrap();
        apply(&p, None, None).unwrap();
    }
}
