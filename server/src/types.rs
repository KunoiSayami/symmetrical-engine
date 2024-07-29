use axum::http::HeaderValue;
use axum_extra::headers::{self, Header};
use once_cell::sync::Lazy;
use serde::Deserialize;

static HEADER_REAL_IP_NAME: Lazy<axum::http::HeaderName> =
    Lazy::new(|| "X-Real-IP".parse().unwrap());

pub struct RealIP(String);

impl Header for RealIP {
    fn name() -> &'static axum::http::HeaderName {
        &HEADER_REAL_IP_NAME
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, axum_extra::headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i axum::http::HeaderValue>,
    {
        let value = values.next().ok_or_else(headers::Error::invalid)?;
        value
            .to_str()
            .map(|s| Self(s.to_string()))
            .map_err(|_| headers::Error::invalid())
    }

    fn encode<E: Extend<axum::http::HeaderValue>>(&self, values: &mut E) {
        let s =
            HeaderValue::from_str(&self.0).unwrap_or_else(|_| HeaderValue::from_static("ERROR"));
        values.extend(std::iter::once(s))
    }
}

impl RealIP {
    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WebBroadcastEvent {
    RequestTerminate(String),
    ServerQuit,
}

impl WebBroadcastEvent {
    pub fn is_not_quit(self) -> bool {
        !self.eq(&Self::ServerQuit)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WebData {
    Auth { uuid: String },
    RequestTerminate,
}

impl TryFrom<&str> for WebData {
    type Error = serde_json::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value)
    }
}
