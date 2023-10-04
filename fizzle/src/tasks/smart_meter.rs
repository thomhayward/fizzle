use fizzle::util::{parse_json_payload, timestamp_ms};
use influxdb::write::buffered::Client as InfluxDbClient;
use mqtt::{clients::tokio::Client as MqttClient, FilterBuf};

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

pub async fn smart_meter_task(
	mqtt_client: MqttClient,
	influxdb_client: InfluxDbClient,
	topic_filter: FilterBuf,
) -> anyhow::Result<()> {
	let mut impulse_context: Option<ImpulseContext> = None;

	let mut impulses = mqtt_client.subscribe(topic_filter.as_str(), 8).await?;
	while let Some(message) = impulses.recv().await {
		//
		// Parse the payload as an Impulse object.
		let payload: Impulse = match parse_json_payload(message) {
			Ok(payload) => payload,
			Err(error) => {
				tracing::error!("error parsing impulse payload: {error:?}");
				continue;
			}
		};

		let context = impulse_context.get_or_insert_with(|| {
			ImpulseContext::with_initial_count(payload.impulse_count as i64)
		});

		if (payload.impulse_count as i64) < context.previous_count {
			tracing::info!("impulse counter reset detected, adjusting offset");
			context.offset = context.previous_count;
		}

		influxdb_client
			.write_with(context.write_line_protocol_with(&payload, &timestamp_ms()))
			.await?;

		// Update the count
		context.previous_count = payload.impulse_count.into();
	}

	Ok(())
}
