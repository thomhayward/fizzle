use std::{borrow, fmt};

use bytes::BytesMut;
use tokio::{
	sync::{mpsc, watch},
	task::JoinHandle,
};

use super::{buffered, LineBuilder, LINE_PROTOCOL_BUFFER_LEN};

#[derive(Debug)]
pub struct Client {
	client: reqwest::Client,
	url: url::Url,
}

impl Client {
	pub(crate) fn new(client: reqwest::Client, url: url::Url) -> Self {
		Self { client, url }
	}

	pub async fn write<B: bytes::Buf>(&self, line_protocol: B) -> Result<(), WriteError>
	where
		B: Into<reqwest::Body>,
	{
		let response = match self
			.client
			.post(self.url.clone())
			.body(line_protocol)
			.send()
			.await
		{
			Ok(response) => response,
			Err(error) => {
				tracing::error!("error sending data to InfluxDB: {error:?}");
				return Err(WriteError);
			}
		};

		let status = response.status();
		if status == 204 {
			Ok(())
		} else {
			let body = response.text().await.unwrap();
			tracing::error!("influxdb response: {body}");
			Err(WriteError)
		}
	}

	pub async fn write_with<F>(&self, f: F) -> Result<(), WriteError>
	where
		F: FnOnce(LineBuilder) -> LineBuilder,
	{
		let buf = BytesMut::with_capacity(LINE_PROTOCOL_BUFFER_LEN);
		let builder = LineBuilder::new_with(buf);
		let buf = f(builder).build().freeze();

		self.write(buf).await
	}

	/// Returns the name of the bucket data is written to.
	pub fn bucket(&self) -> borrow::Cow<'_, str> {
		let (_, bucket) = self
			.url
			.query_pairs()
			.find(|(key, _)| key == "bucket")
			.expect("bucket query parameter should be set");

		bucket
	}

	/// Creates a buffered client with the default options.
	pub fn buffered(
		self,
		shutdown_signal: watch::Receiver<bool>,
	) -> (buffered::Client, JoinHandle<anyhow::Result<()>>) {
		self.buffered_with(shutdown_signal, Default::default())
	}

	pub fn buffered_with(
		self,
		shutdown_signal: watch::Receiver<bool>,
		options: buffered::Options,
	) -> (buffered::Client, JoinHandle<anyhow::Result<()>>) {
		let (tx, rx) = mpsc::channel(options.channel_len);

		let handle = tokio::spawn(buffered::buffered_write_task(
			self,
			rx,
			shutdown_signal,
			options,
		));
		let client = buffered::Client::new(tx);

		(client, handle)
	}
}

#[derive(Debug)]
pub struct WriteError;

impl fmt::Display for WriteError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{self:?}")
	}
}

impl std::error::Error for WriteError {}
