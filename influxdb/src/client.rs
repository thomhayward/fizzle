use crate::write::builder::Builder;
use reqwest::{
	header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
	ClientBuilder, IntoUrl,
};
use url::Url;

#[derive(Debug)]
pub struct Client {
	client: reqwest::Client,
	host: Url,
}

impl Client {
	/// Creates a new InfluxDB client.
	///
	/// # Arguments
	/// * `host` - The URL of the InfluxDB host.
	/// * `token` - The token to use for authentication.
	///
	/// # Errors
	/// Returns an error if the URL is invalid, or the token does not serialize
	/// to a valid header value.
	///
	pub fn new(
		host: impl IntoUrl,
		token: impl AsRef<str>,
	) -> Result<Self, Box<dyn std::error::Error + 'static>> {
		let host = host.into_url()?;
		let token = token.as_ref();

		// Create the default header set.
		//
		let mut default_headers = HeaderMap::new();
		let mut authorization_header = HeaderValue::from_str(&format!("Token {token}"))?;
		authorization_header.set_sensitive(true);

		default_headers.insert(AUTHORIZATION, authorization_header);
		default_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
		default_headers.insert(
			CONTENT_TYPE,
			HeaderValue::from_static("text/plain; charset=utf-8"),
		);

		// Build the HTTP client. This will be reused for all requests.
		//
		let client = ClientBuilder::new()
			.gzip(true)
			.default_headers(default_headers)
			.build()?;

		Ok(Self { host, client })
	}

	/// Creates a write client builder for the given bucket.
	pub fn write_to_bucket(&self, bucket: impl Into<String>) -> Builder {
		Builder::new_with(self.client.clone(), self.host.clone(), bucket.into())
	}

	pub fn query(&self) {
		unimplemented!()
	}

	/// Returns the URL of the InfluxDB host.
	pub fn host(&self) -> &Url {
		&self.host
	}
}
