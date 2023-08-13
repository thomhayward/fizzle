use super::{immediate, LineBuilder, Status, LINE_PROTOCOL_BUFFER_LEN};
use bytes::{Bytes, BytesMut};
use core::fmt;
use std::{collections::VecDeque, time::Duration};
use tokio::{
	sync::{mpsc, watch},
	time::interval,
};

const DEFAULT_LINE_LIMIT: usize = 5000;

#[derive(Clone, Debug)]
pub struct Client {
	channel: mpsc::Sender<(Bytes, watch::Sender<Status>)>,
}

#[derive(Debug)]
pub struct Options {
	pub channel_len: usize,
	pub max_timeout: Duration,
	pub max_lines: usize,
}

impl Default for Options {
	fn default() -> Self {
		Self {
			channel_len: 64,
			max_timeout: Duration::from_secs(60),
			max_lines: DEFAULT_LINE_LIMIT,
		}
	}
}

#[derive(Debug)]
pub struct BufferedWriteError;

impl fmt::Display for BufferedWriteError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{self:?}")
	}
}

impl std::error::Error for BufferedWriteError {}

impl Client {
	pub(crate) fn new(channel: mpsc::Sender<(Bytes, watch::Sender<Status>)>) -> Self {
		Self { channel }
	}

	pub async fn write_with<F>(&self, f: F) -> Result<watch::Receiver<Status>, BufferedWriteError>
	where
		F: FnOnce(LineBuilder) -> LineBuilder,
	{
		let buf = BytesMut::with_capacity(LINE_PROTOCOL_BUFFER_LEN);
		let builder = LineBuilder::new_with(buf);
		let buf = f(builder).build().freeze();

		let (tx, rx) = watch::channel(Status::Init);
		self.channel
			.send((buf, tx))
			.await
			.map_err(|_| BufferedWriteError)?;

		Ok(rx)
	}
}

impl Drop for Client {
	fn drop(&mut self) {
		self.channel.downgrade();
	}
}

pub async fn buffered_write_task(
	client: immediate::Client,
	mut channel: mpsc::Receiver<(Bytes, watch::Sender<Status>)>,
	options: Options,
) -> anyhow::Result<()> {
	let mut shutdown = false;

	let mut lines = 0;
	let mut buffers = VecDeque::new();

	let mut flush_interval = interval(options.max_timeout);

	while !shutdown {
		let flush = tokio::select! {
			biased;

			message = channel.recv() => {
				match message {
					Some((buffer, status)) => {
						// Calculate how many lines we've received.
						let new_lines = buffer.iter().filter(|&&x| x == b'\n').count();
						lines += new_lines;

						let len = buffer.len();
						status.send_replace(Status::Buffered);
						buffers.push_back((buffer, status));

						tracing::trace!(
							"buffering {new_lines} lines, {len} bytes of line-protocol; {} entries in buffers, {lines} lines",
							buffers.len()
						);

						// Flush the buffers immediately if we've already reached the limit.
						lines >= options.max_lines
					}
					None => {
						tracing::debug!("channel closed, shutting down task");
						shutdown = true;
						true
					}
				}
			}
			_ = flush_interval.tick() => {
				!buffers.is_empty()
			}
			else => {
				shutdown = true;
				!buffers.is_empty()
			}
		};

		if flush {
			tracing::debug!("will send buffered line-protocol to InfluxDB instance");

			let mut in_progress = VecDeque::new();
			let mut body_buffer = BytesMut::new();
			let mut total_lines = 0;

			while let Some((buffer, status)) = buffers.pop_front() {
				let new_lines = buffer.iter().filter(|&&x| x == b'\n').count();
				total_lines += new_lines;

				body_buffer.extend_from_slice(&buffer);
				in_progress.push_back((buffer, status));
				if total_lines >= options.max_lines {
					break;
				}
			}

			match client.write(body_buffer.freeze()).await {
				Ok(_) => {
					tracing::debug!(
						"wrote {} lines to bucket '{}'",
						total_lines,
						client.bucket()
					);
					lines -= total_lines;
					for (_, status) in in_progress {
						status.send_replace(Status::Accepted);
					}
				}
				Err(error) => {
					tracing::error!("error submitting line protocol: {error:?}");
					for value in in_progress {
						buffers.push_front(value);
					}
				}
			}
		}
	}

	tracing::debug!(
		"buffered client task for bucket '{}' stopped",
		client.bucket()
	);

	Ok(())
}
