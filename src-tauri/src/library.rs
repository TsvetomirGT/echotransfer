use std::collections::HashMap;

use futures::{stream, StreamExt, TryStreamExt};
use serde::Serialize;

use crate::error::Result;
use crate::subsonic::{Album, Client, Song};

/// Full server library cached in memory after connect.
#[derive(Debug, Default)]
pub struct Library {
    pub albums: HashMap<String, Album>,
    /// album id -> songs sorted by disc/track
    pub songs_by_album: HashMap<String, Vec<Song>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistView {
    pub id: String,
    pub name: String,
    pub albums: Vec<AlbumView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumView {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub year: Option<u32>,
    pub cover_art: Option<String>,
    pub song_count: usize,
    pub size: u64,
    pub duration: u64,
}

impl Library {
    pub async fn fetch(client: &Client) -> Result<Self> {
        let (albums, mut songs) = tokio::try_join!(client.all_albums(), client.all_songs())?;
        // Fallback for servers without empty-query search3 support.
        if songs.is_empty() && !albums.is_empty() {
            let ids: Vec<String> = albums.iter().map(|a| a.id.clone()).collect();
            songs = stream::iter(ids)
                .map(|id| async move { client.album_songs(&id).await })
                .buffer_unordered(8)
                .try_concat()
                .await?;
        }
        Ok(Self::from_parts(albums, songs))
    }

    pub fn from_parts(albums: Vec<Album>, songs: Vec<Song>) -> Self {
        let mut songs_by_album: HashMap<String, Vec<Song>> = HashMap::new();
        for s in songs {
            if let Some(aid) = s.album_id.clone() {
                songs_by_album.entry(aid).or_default().push(s);
            }
        }
        for list in songs_by_album.values_mut() {
            list.sort_by_key(|s| (s.disc_number.unwrap_or(1), s.track.unwrap_or(0)));
        }
        Self {
            albums: albums.into_iter().map(|a| (a.id.clone(), a)).collect(),
            songs_by_album,
        }
    }

    pub fn songs(&self, album_id: &str) -> &[Song] {
        self.songs_by_album.get(album_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Artists (album artists) sorted by name, each with sorted albums.
    pub fn artists(&self) -> Vec<ArtistView> {
        let mut by_artist: HashMap<String, Vec<AlbumView>> = HashMap::new();
        for a in self.albums.values() {
            let songs = self.songs(&a.id);
            if songs.is_empty() {
                continue;
            }
            let name = a.artist.clone().unwrap_or_else(|| "Unknown Artist".into());
            let id = a.artist_id.clone().unwrap_or_else(|| format!("name:{name}"));
            by_artist
                .entry(id)
                .or_default()
                .push(AlbumView {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    artist: name,
                    year: a.year.filter(|y| *y > 0),
                    cover_art: a.cover_art.clone(),
                    song_count: songs.len(),
                    size: songs.iter().map(|s| s.size).sum(),
                    duration: songs.iter().map(|s| s.duration).sum(),
                });
        }
        let mut out: Vec<ArtistView> = by_artist
            .into_iter()
            .map(|(id, mut albums)| {
                albums.sort_by(|a, b| a.year.cmp(&b.year).then_with(|| a.name.cmp(&b.name)));
                ArtistView {
                    id,
                    name: albums[0].artist.clone(),
                    albums,
                }
            })
            .collect();
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artists_unique_by_id_even_with_name_variants() {
        let album = |id: &str, artist: &str| Album {
            id: id.into(),
            name: id.into(),
            artist: Some(artist.into()),
            artist_id: Some("ar1".into()),
            ..Default::default()
        };
        let song = |id: &str, al: &str| Song { id: id.into(), album_id: Some(al.into()), ..Default::default() };
        let lib = Library::from_parts(
            vec![album("a1", "Sigur Rós"), album("a2", "sigur rós")],
            vec![song("s1", "a1"), song("s2", "a2")],
        );
        let artists = lib.artists();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].albums.len(), 2);
    }
}
