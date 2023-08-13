use bytes::BytesMut;
use influxdb_line_protocol::{builder::BeforeMeasurement, LineProtocolBuilder};

pub mod buffered;
pub mod builder;
pub mod immediate;
pub mod precision;

pub type LineBuilder = LineProtocolBuilder<BytesMut, BeforeMeasurement>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
	Init,
	Buffered,
	Accepted,
}

/// Initial size of the buffer to use with LineProtocolBuilder instances.
pub const LINE_PROTOCOL_BUFFER_LEN: usize = 1024;
