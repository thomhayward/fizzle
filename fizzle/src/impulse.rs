use influxdb::LineBuilder;
use serde::Deserialize;
use std::time::Instant;

#[derive(Clone, Debug, Deserialize)]
pub struct Impulse {
	/// The number of Watt hours since the meter was set up.
	pub impulse_count: u32,
	/// The number of microseconds since the meter was set up.
	pub clock: u64,
	/// The number of microseconds between impulses.
	pub interval: u32,
	/// The rate of power consumption since the previous impulse in Watts.
	pub power: f32,
}

#[derive(Debug, Clone)]
pub struct ImpulseContext {
	pub previous_count: i64,
	pub offset: i64,
	pub first_impulse: Instant,
}

impl ImpulseContext {
	pub fn with_initial_count(count: i64) -> Self {
		Self {
			previous_count: count,
			offset: count,
			first_impulse: Instant::now(),
		}
	}

	pub fn write_line_protocol_with<'a>(
		&'a self,
		impulse: &'a Impulse,
		timestamp: &'a i64,
	) -> impl FnOnce(LineBuilder) -> LineBuilder + 'a {
		|builder| {
			builder
				.measurement("impulse")
				.tag("device", "garage/meter")
				.field("device_uptime", impulse.clock / 1_000_000)
				.field("energy", impulse.impulse_count as i64 - self.offset + 1)
				.field("monitor_uptime", self.first_impulse.elapsed().as_secs())
				.field("power", impulse.power.round() as i64)
				.timestamp(*timestamp)
				.close_line()
		}
	}
}
