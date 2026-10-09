use std::time::Duration;

use md5::{Digest, Md5};
use rand::Rng;
use serde::de::DeserializeOwned;
use url::Url;

use super::models::*;
use crate::error::{Error, Result};

const API_VERSION: &str = "1.16.1";
const CLIENT_NAME: &str = "echotransfer";
const SEARCH_PAGE: usize = 1000;
const ALBUM_PAGE: usize = 500;

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: Url,
    user: String,
    password: String,
}

impl Client {
    pub fn new(server_url: &str, user: &str, password: &str) -> Result<Self> {
        let mut raw = server_url.trim().to_string();
        if !raw.starts_with("http://") && !raw.starts_with("https://") {
            raw = format!("https://{raw}");
        }
        if !raw.ends_with('/') {
            raw.push('/');
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .user_agent(concat!("EchoTransfer/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            base: Url::parse(&raw)?,
            user: user.to_string(),
            password: password.to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }

    /// Builds an authenticated endpoint URL (token auth: t = md5(password + salt)).
    pub fn endpoint_url(&self, endpoint: &str, params: &[(&str, String)]) -> Url {
        let salt: String = {
            let bytes: [u8; 8] = rand::rng().random();
            hex::encode(bytes)
        };
        let token = hex::encode(Md5::digest(format!("{}{}", self.password, salt)));
        let mut url = self
            .base
            .join(&format!("rest/{endpoint}"))
            .expect("static endpoint path");
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("u", &self.user)
                .append_pair("t", &token)
                .append_pair("s", &salt)
                .append_pair("v", API_VERSION)
                .append_pair("c", CLIENT_NAME)
                .append_pair("f", "json");
            for (k, v) in params {
                q.append_pair(k, v);
            }
        }
        url
    }

    async fn get<T: DeserializeOwned + Default>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> Result<Response<T>> {
        let url = self.endpoint_url(endpoint, params);
        let resp = self.http.get(url).send().await?.error_for_status()?;
        let env: Envelope<T> = resp.json().await?;
        let r = env.response;
        if r.status != "ok" {
            let e = r.error.unwrap_or(ApiError {
                code: -1,
                message: "unknown error".into(),
            });
            return Err(Error::Subsonic {
                code: e.code,
                message: e.message,
            });
        }
        Ok(r)
    }

    /// Returns server description, e.g. "navidrome 0.53.3".
    pub async fn ping(&self) -> Result<String> {
        let r = self.get::<Empty>("ping", &[]).await?;
        Ok(match (r.server_type, r.server_version) {
            (Some(t), Some(v)) => format!("{t} {v}"),
            (Some(t), None) => t,
            _ => format!("Subsonic API {}", r.version),
        })
    }

    pub async fn all_albums(&self) -> Result<Vec<Album>> {
        let mut out = Vec::new();
        loop {
            let r = self
                .get::<AlbumList2Body>(
                    "getAlbumList2",
                    &[
                        ("type", "alphabeticalByArtist".into()),
                        ("size", ALBUM_PAGE.to_string()),
                        ("offset", out.len().to_string()),
                    ],
                )
                .await?;
            let page = r.body.album_list2.album;
            let n = page.len();
            out.extend(page);
            if n < ALBUM_PAGE {
                return Ok(out);
            }
        }
    }

    /// All songs via paged empty search3 (supported by Navidrome).
    pub async fn all_songs(&self) -> Result<Vec<Song>> {
        let mut out = Vec::new();
        loop {
            let r = self
                .get::<Search3Body>(
                    "search3",
                    &[
                        ("query", String::new()),
                        ("artistCount", "0".into()),
                        ("albumCount", "0".into()),
                        ("songCount", SEARCH_PAGE.to_string()),
                        ("songOffset", out.len().to_string()),
                    ],
                )
                .await?;
            let page = r.body.search_result3.song;
            let n = page.len();
            out.extend(page);
            if n < SEARCH_PAGE {
                return Ok(out);
            }
        }
    }

    pub async fn album_songs(&self, album_id: &str) -> Result<Vec<Song>> {
        let r = self
            .get::<AlbumBody>("getAlbum", &[("id", album_id.to_string())])
            .await?;
        Ok(r.body.album.map(|a| a.song).unwrap_or_default())
    }

    /// Request for the audio bytes: original file, or server-side transcode.
    pub fn media_request(&self, song_id: &str, transcode: Option<(&str, u32)>) -> reqwest::RequestBuilder {
        let url = match transcode {
            None => self.endpoint_url("download", &[("id", song_id.to_string())]),
            Some((format, bitrate)) => self.endpoint_url(
                "stream",
                &[
                    ("id", song_id.to_string()),
                    ("format", format.to_string()),
                    ("maxBitRate", bitrate.to_string()),
                ],
            ),
        };
        // Downloads can be large; no overall timeout, only read timeout.
        self.http.get(url)
    }

    pub async fn cover_art(&self, id: &str, size: u32) -> Result<Vec<u8>> {
        let url = self.endpoint_url("getCoverArt", &[("id", id.to_string()), ("size", size.to_string())]);
        let resp = self.http.get(url).send().await?.error_for_status()?;
        let is_json = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.contains("json") || v.contains("xml"));
        if is_json {
            return Err(Error::Other(format!("no cover art for {id}")));
        }
        Ok(resp.bytes().await?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ok(body: serde_json::Value) -> ResponseTemplate {
        let mut inner = serde_json::json!({"status": "ok", "version": "1.16.1", "type": "navidrome", "serverVersion": "0.53.3"});
        inner.as_object_mut().unwrap().extend(body.as_object().unwrap().clone());
        ResponseTemplate::new(200).set_body_json(serde_json::json!({ "subsonic-response": inner }))
    }

    #[tokio::test]
    async fn ping_reports_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/ping"))
            .and(query_param("u", "alice"))
            .and(query_param("f", "json"))
            .respond_with(ok(serde_json::json!({})))
            .mount(&server)
            .await;
        let c = Client::new(&server.uri(), "alice", "pw").unwrap();
        assert_eq!(c.ping().await.unwrap(), "navidrome 0.53.3");
    }

    #[tokio::test]
    async fn ping_maps_api_error() {
        let server = MockServer::start().await;
        Mock::given(path("/rest/ping"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "subsonic-response": {"status": "failed", "version": "1.16.1",
                    "error": {"code": 40, "message": "Wrong username or password"}}
            })))
            .mount(&server)
            .await;
        let c = Client::new(&server.uri(), "alice", "bad").unwrap();
        match c.ping().await {
            Err(Error::Subsonic { code, .. }) => assert_eq!(code, 40),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn token_is_md5_of_password_and_salt() {
        let c = Client::new("http://x", "u", "sesame").unwrap();
        let url = c.endpoint_url("ping", &[]);
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        let expect = hex::encode(Md5::digest(format!("sesame{}", q["s"])));
        assert_eq!(q["t"], expect);
        assert!(!url.as_str().contains("sesame"));
    }

    #[tokio::test]
    async fn all_songs_pages_until_short_page() {
        let server = MockServer::start().await;
        let page1: Vec<_> = (0..SEARCH_PAGE)
            .map(|i| serde_json::json!({"id": format!("s{i}"), "title": "t", "size": 10, "suffix": "mp3"}))
            .collect();
        Mock::given(path("/rest/search3"))
            .and(query_param("songOffset", "0"))
            .respond_with(ok(serde_json::json!({"searchResult3": {"song": page1}})))
            .mount(&server)
            .await;
        Mock::given(path("/rest/search3"))
            .and(query_param("songOffset", SEARCH_PAGE.to_string()))
            .respond_with(ok(serde_json::json!({"searchResult3": {"song": [{"id": "last", "title": "x"}]}})))
            .mount(&server)
            .await;
        let c = Client::new(&server.uri(), "u", "p").unwrap();
        let songs = c.all_songs().await.unwrap();
        assert_eq!(songs.len(), SEARCH_PAGE + 1);
        assert_eq!(songs.last().unwrap().id, "last");
    }

    #[tokio::test]
    async fn empty_search_result_is_ok() {
        let server = MockServer::start().await;
        Mock::given(path("/rest/search3"))
            .respond_with(ok(serde_json::json!({"searchResult3": {}})))
            .mount(&server)
            .await;
        let c = Client::new(&server.uri(), "u", "p").unwrap();
        assert!(c.all_songs().await.unwrap().is_empty());
    }

    #[test]
    fn base_url_normalized() {
        let c = Client::new("music.example.com/navidrome", "u", "p").unwrap();
        assert_eq!(c.base_url(), "https://music.example.com/navidrome/");
        assert!(c.endpoint_url("ping", &[]).as_str().starts_with("https://music.example.com/navidrome/rest/ping?"));
    }
}
