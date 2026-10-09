use serde::{Deserialize, Serialize};

/// Top-level `{"subsonic-response": {...}}` wrapper.
#[derive(Debug, Deserialize)]
pub struct Envelope<T> {
    #[serde(rename = "subsonic-response")]
    pub response: Response<T>,
}

#[derive(Debug, Deserialize)]
pub struct Response<T> {
    pub status: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "type")]
    pub server_type: Option<String>,
    #[serde(default, rename = "serverVersion")]
    pub server_version: Option<String>,
    pub error: Option<ApiError>,
    #[serde(flatten)]
    pub body: T,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub code: i64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Empty {}

#[derive(Debug, Default, Deserialize)]
pub struct AlbumList2Body {
    #[serde(default, rename = "albumList2")]
    pub album_list2: AlbumList2,
}

#[derive(Debug, Default, Deserialize)]
pub struct AlbumList2 {
    #[serde(default)]
    pub album: Vec<Album>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Search3Body {
    #[serde(default, rename = "searchResult3")]
    pub search_result3: SearchResult3,
}

#[derive(Debug, Default, Deserialize)]
pub struct SearchResult3 {
    #[serde(default)]
    pub song: Vec<Song>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AlbumBody {
    pub album: Option<AlbumWithSongs>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AlbumWithSongs {
    #[serde(default)]
    pub song: Vec<Song>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub artist_id: Option<String>,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    /// OpenSubsonic extension (Navidrome sends it).
    #[serde(default)]
    pub display_album_artist: Option<String>,
    #[serde(default)]
    pub track: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub bit_rate: Option<u32>,
}
