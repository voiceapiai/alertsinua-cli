//! The client implementation for the reqwest HTTP client, which is async
//! @borrows https://github.com/ramsayleung/rspotify/blob/master/rspotify-http/src/reqwest.rs

use reqwest::{Method, RequestBuilder, StatusCode};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::ApiError;

pub type Headers = HashMap<String, String>;
pub type Query<'a> = HashMap<&'a str, &'a str>;

pub const API_BASE_URL: &str = "https://api.alerts.in.ua";
pub const API_VERSION: &str = "/v1";
pub const API_ALERTS_ACTIVE: &str = "/alerts/active.json";
pub const API_ALERTS_ACTIVE_BY_REGION_STRING: &str = "/iot/active_air_raid_alerts_by_oblast.json";

#[derive(Debug, Clone)]
pub struct AlertsInUaClient {
    base_url: String,
    token: String,
    client: reqwest::Client,
}

impl AlertsInUaClient {
    #[rustfmt::skip]
    pub fn new<U, T>(base_url: U, token: T) -> Self where U: Into<String>, T: Into<String>,
    {
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()
            // building with these options cannot fail
            .unwrap();
        Self {
            base_url: base_url.into(),
            token: token.into(),
            client,
        }
    }
}

impl AlertsInUaClient {
    fn get_api_url(&self, url: &str) -> String {
        let version = API_VERSION;
        let base_url = self.base_url.clone();
        // if !base_url.ends_with('/') { base_url.push('/'); }
        base_url + version + url
    }

    async fn request<R, D>(&self, method: Method, url: &str, add_data: D) -> Result<R, ApiError>
    where
        R: for<'de> Deserialize<'de>,
        D: Fn(RequestBuilder) -> RequestBuilder,
    {
        // Build full URL
        let url = self.get_api_url(url);
        let mut request = self.client.request(method.clone(), url);
        // Enable HTTP bearer authentication.
        request = request.bearer_auth(&self.token);

        // Configuring the request for the specific type (get/post/put/delete)
        request = add_data(request);

        // Finally performing the request and handling the response
        // log::info!("Making request {:?}", request);
        let response = request.send().await?;

        // Making sure that the status code is OK

        match response.error_for_status() {
            Ok(res) => res.json::<R>().await.map_err(Into::into),
            Err(err) => match err.status() {
                Some(StatusCode::BAD_REQUEST) => Err(ApiError::InvalidParameterException),
                Some(StatusCode::UNAUTHORIZED) => Err(ApiError::UnauthorizedError(err)),
                Some(StatusCode::FORBIDDEN) => Err(ApiError::ForbiddenError),
                Some(StatusCode::TOO_MANY_REQUESTS) => Err(ApiError::RateLimitError),
                Some(StatusCode::INTERNAL_SERVER_ERROR) => Err(ApiError::InternalServerError),
                _ => Err(ApiError::Unknown(err)),
            },
        }
    }
}

/// This trait represents the interface to be implemented for an HTTP client,
/// which is kept separate from the Spotify client for cleaner code. Thus, it
/// also requires other basic traits that are needed for the Spotify client.
///
/// When a request doesn't need to pass parameters, the empty or default value
/// of the payload type should be passed, like `json!({})` or `Query::new()`.
/// This avoids using `Option<T>` because `Value` itself may be null in other
/// different ways (`Value::Null`, an empty `Value::Object`...), so this removes
/// redundancy and edge cases (a `Some(Value::Null), for example, doesn't make
/// much sense).
// #[cfg_attr(target_arch = "wasm32", maybe_async(?Send))]
// #[cfg_attr(not(target_arch = "wasm32"), maybe_async)]
pub trait BaseHttpClient: Send + Clone + fmt::Debug {
    type Error;

    // This internal function should always be given an object value in JSON.
    #[allow(async_fn_in_trait)]
    async fn get<R>(&self, url: &str, payload: Option<&Query>) -> Result<R, Self::Error>
    where
        R: for<'de> Deserialize<'de>;
}

// #[cfg_attr(target_arch = "wasm32", async_impl(?Send))]
// #[cfg_attr(not(target_arch = "wasm32"), async_impl)]
impl BaseHttpClient for AlertsInUaClient {
    type Error = ApiError;

    #[inline]
    async fn get<R>(&self, url: &str, _payload: Option<&Query<'_>>) -> Result<R, Self::Error>
    where
        R: for<'de> Deserialize<'de>,
    {
        // self.request(Method::GET, url, |req| req.query(payload))
        self.request(Method::GET, url, |r| r).await
    }
}

// =============================================================================