use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Server error {code}: {message}")]
    Subsonic { code: i64, message: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Tag read error: {0}")]
    TagRead(#[from] lofty::error::FileParseError),
    #[error("Tag write error: {0}")]
    TagWrite(#[from] lofty::error::FileEncodingError),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Keychain error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Not connected to a server")]
    NotConnected,
    #[error("Cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

// Commands return errors to the UI as plain strings.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
