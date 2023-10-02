use bytes::{Buf, Bytes};
use mqtt::clients::tokio::Message;
use time::OffsetDateTime;

pub fn parse_json_payload<T: serde::de::DeserializeOwned>(
	message: Message,
) -> serde_json::Result<T> {
	let topic = message.topic;
	let reader = message.payload.reader();
	match serde_json::from_reader(reader) {
		Ok(v) => Ok(v),
		Err(error) => {
			tracing::error!("failed to deserialise payload from '{topic}': {}", error);
			Err(error)
		}
	}
}

#[inline]
pub fn timestamp_ms() -> i64 {
	millis_from_datetime(OffsetDateTime::now_utc())
}

#[inline]
pub fn millis_from_datetime(dt: OffsetDateTime) -> i64 {
	let timestamp = dt.unix_timestamp_nanos() / 1_000_000;
	timestamp
		.try_into()
		.expect("timestamp in milliseconds shouldn't overflow an i64")
}

pub fn bytes_to_string(bytes: Bytes) -> Result<String, std::io::Error> {
	use std::io::Read;

	let mut line = String::with_capacity(bytes.len());
	let mut reader = bytes.reader();
	reader.read_to_string(&mut line)?;
	Ok(line)
}
