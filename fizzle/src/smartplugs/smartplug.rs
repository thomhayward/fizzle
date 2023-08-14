use crate::util::millis_from_datetime;
use std::{collections::BTreeMap, error, fmt, time::Instant};
use tasmota::{sns::StatusSNS, PowerState, StatusSTS};
use time::OffsetDateTime;

use super::topic::{TelemetryType, TopicGenerator};

#[derive(Debug)]
pub struct SmartPlug<G: TopicGenerator> {
	name: String,

	lwt: Option<String>,
	raw_telemetry: BTreeMap<OffsetDateTime, (Option<StatusSNS>, Option<StatusSTS>)>,
	last_energy: Option<f32>,
	energy_offset: f32,
	first_observation: Instant,

	_phantom: std::marker::PhantomData<G>,
}

#[derive(Debug)]
pub struct TelemetryNotAvailable(TelemetryType);

impl fmt::Display for TelemetryNotAvailable {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{self:?}")
	}
}

impl error::Error for TelemetryNotAvailable {}

impl<G: TopicGenerator> SmartPlug<G> {
	/// Creates a new smart plug with the given name.
	pub fn new(name: String) -> Self {
		Self {
			name,
			lwt: None,
			raw_telemetry: Default::default(),
			last_energy: None,
			energy_offset: 0f32,
			first_observation: Instant::now(),
			_phantom: std::marker::PhantomData,
		}
	}

	/// Returns the name of the smart plug.
	#[inline(always)]
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Generates the MQTT topic for the smart plug's sensor telemetry.
	pub fn sensor_telemetry_topic(&self) -> String {
		G::sensor_telemetry_topic(&self.name)
	}

	/// Generates the MQTT topic for the smart plug's state telemetry.
	pub fn state_telemetry_topic(&self) -> String {
		G::state_telemetry_topic(&self.name)
	}

	/// Generates the MQTT topic for the smart plug's last will and testament.
	pub fn lwt_topic(&self) -> String {
		G::lwt_topic(&self.name)
	}

	/// Returns the last will and testament of the smart plug, if any.
	#[allow(dead_code)]
	pub fn lwt(&self) -> Option<&str> {
		self.lwt.as_deref()
	}

	pub fn set_lwt(&mut self, lwt: String) -> Option<String> {
		tracing::trace!(
			"smartplug '{}', setting last will and testament: '{}'",
			self.name(),
			lwt
		);
		self.lwt.replace(lwt)
	}

	pub fn append_sensor_telemetry(&mut self, telemetry: StatusSNS) {
		let timestamp = telemetry.time.assume_utc();

		if let Some(last) = self.raw_telemetry.last_entry() {
			if last.key() > &timestamp {
				tracing::warn!(
					"new telemetry has an older timestamp than previous telemetry, discarding"
				);
				return;
			}
		}

		self.energy_offset = self
			.last_energy
			.map(|value| {
				if telemetry.energy.energy_lifetime < value {
					tracing::warn!("energy counter reset detected for device '{}'", self.name);
					value
				} else {
					self.energy_offset
				}
			})
			.unwrap_or(telemetry.energy.energy_lifetime);
		self.last_energy = Some(telemetry.energy.energy_lifetime);

		let (sns, _) = self.raw_telemetry.entry(timestamp).or_default();
		if let Some(old_telemetry) = sns.replace(telemetry.clone()) {
			tracing::warn!("received SNS telemetry with duplicate timestamp: {old_telemetry:?}");
		}
	}

	pub fn append_state_telemetry(&mut self, telemetry: StatusSTS) {
		let timestamp = telemetry.time.assume_utc();

		let (_, sts) = self.raw_telemetry.entry(timestamp).or_default();
		if let Some(old_telemetry) = sts.replace(telemetry.clone()) {
			tracing::warn!("received STS telemetry with duplicate timestamp: {old_telemetry:?}");
		}
	}

	pub fn first_matched_telemetry(&mut self) -> Option<(OffsetDateTime, StatusSNS, StatusSTS)> {
		let key = self
			.raw_telemetry
			.iter()
			.find(|(_, (sns, sts))| sns.is_some() && sts.is_some())
			.map(|(key, _)| *key);

		key.and_then(|key| {
			self.raw_telemetry
				.remove(&key)
				.map(|(sns, sts)| (key, sns.unwrap(), sts.unwrap()))
		})
	}

	/// Removes the oldest matched sensor and state telemetry.
	pub fn matched_telemetry(&mut self) -> Option<(OffsetDateTime, StatusSNS, StatusSTS)> {
		self.first_matched_telemetry()
	}

	pub fn generate_telemetry(
		&self,
		odt: OffsetDateTime,
		sensor: StatusSNS,
		state: StatusSTS,
	) -> Result<Telemetry, TelemetryNotAvailable> {
		let monitor_uptime = self.first_observation.elapsed().as_secs();
		let energy = ((sensor.energy.energy_lifetime - self.energy_offset) * 1000.0).round() as i64;

		// Pick the timestamp to use for the telemetry datum.
		let device_timestamp = millis_from_datetime(state.time.assume_utc());
		let machine_timestamp = millis_from_datetime(odt);
		let drift = machine_timestamp.abs_diff(device_timestamp);
		let timestamp = if drift > 20_000 {
			tracing::warn!(
				"timestamp drift for '{}' is {}ms > 20,000ms, using machine time",
				self.name,
				drift
			);
			machine_timestamp
		} else {
			device_timestamp
		};

		Ok(Telemetry {
			name: self.name.clone(),
			apparent_power: sensor.energy.apparent_power as i64,
			current: sensor.energy.current as f64,
			device_uptime: state.uptime_seconds,
			energy,
			monitor_uptime,
			power: sensor.energy.power as i64,
			power_factor: sensor.energy.power_factor as f64,
			reactive_power: sensor.energy.reactive_power as i64,
			state: state.power_state,
			voltage: sensor.energy.voltage as i64,
			timestamp,
		})
	}
}

#[derive(Debug)]
pub struct Telemetry {
	pub name: String,
	pub apparent_power: i64,
	pub current: f64,
	pub device_uptime: u64,
	pub energy: i64,
	pub monitor_uptime: u64,
	pub power: i64,
	pub power_factor: f64,
	pub reactive_power: i64,
	pub state: PowerState,
	pub voltage: i64,
	pub timestamp: i64,
}
