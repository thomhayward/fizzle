use crate::{smartplugs::topic::TelemetryType, util::millis_from_datetime};
use influxdb::{buffered, Status};
use regex::Regex;
use rumqttc::Publish;
use std::{collections::BTreeMap, fmt, time::Duration};
use tasmota::PowerState;
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio::sync::watch;

pub trait TopicScheme {
	/// Get the device id from any topic the device would publish to.
	fn get_device_id<'a>(&self, topic: &'a str) -> Option<&'a str>;

	/// Extracts the telemetry type from the topic.
	fn get_telemetry_type(&self, topic: &str) -> Option<TelemetryType>;
}

#[derive(Debug)]
pub struct RegexTopicScheme {
	device_id_re: Regex,
	telemetry_type_re: Regex,
}

impl RegexTopicScheme {
	/// Creates a new topic scheme for extracting device ID's and telemetry type
	/// with the supplied regular expressions.
	///
	/// `device_id_re` should capture the device's ID in a capture group named 'device_id'.
	/// `telemetry_type_re` should capture the telemetry type in a capture group named 'telemetry_type'.
	pub fn new(device_id_re: &str, telemetry_type_re: &str) -> Result<Self, regex::Error> {
		let device_id_re = Regex::new(device_id_re)?;
		let telemetry_type_re = Regex::new(telemetry_type_re)?;
		Ok(Self {
			device_id_re,
			telemetry_type_re,
		})
	}
}

impl TopicScheme for RegexTopicScheme {
	#[tracing::instrument]
	fn get_device_id<'a>(&self, topic: &'a str) -> Option<&'a str> {
		let id = self
			.device_id_re
			.captures(topic)
			.and_then(|capture| capture.name("device_id"))
			.map(|mat| mat.as_str());

		if id.is_none() {
			tracing::error!("failed to extract device ID");
		}
		id
	}

	#[tracing::instrument]
	fn get_telemetry_type(&self, topic: &str) -> Option<TelemetryType> {
		self.telemetry_type_re
			.captures(topic)
			.and_then(|captures| captures.name("telemetry_type"))
			.and_then(|m| match m.as_str() {
				"SENSOR" => Some(TelemetryType::Sensor),
				"STATE" => Some(TelemetryType::State),
				"LWT" => None, // We don't care about LWT.
				value => {
					tracing::debug!("unknown telemetry type: {value}");
					None
				}
			})
	}
}

#[derive(Debug)]
pub struct TasmotaDeviceManager<Scheme> {
	devices: Vec<TasmotaDevice>,
	scheme: Scheme,
}

impl<Scheme> TasmotaDeviceManager<Scheme> {
	pub fn new(scheme: Scheme) -> Self {
		Self {
			devices: Default::default(),
			scheme,
		}
	}
}

impl<Scheme: TopicScheme + fmt::Debug> TasmotaDeviceManager<Scheme> {
	pub fn handle_message(&mut self, message: Publish) {
		let Publish { topic, payload, .. } = message;

		// Attempt to identify the device from the topic string.
		if let Some(id) = self.scheme.get_device_id(&topic) {
			//
			let index = match self
				.devices
				.binary_search_by_key(&id, |device| device.id.as_str())
			{
				Ok(position) => position,
				Err(position) => {
					tracing::debug!("registering new Tasmota device with id='{id}'");
					let device = TasmotaDevice {
						id: id.into(),
						..Default::default()
					};
					self.devices.insert(position, device);
					position
				}
			};

			let device = self.devices.get_mut(index).unwrap();
			match self.scheme.get_telemetry_type(&topic) {
				Some(TelemetryType::Sensor) => {
					let Ok(sns) = serde_json::from_slice(&payload) else {
						tracing::error!("error deserializing StatusSNS payload from device '{}': {payload:?}", device.id);
						return;
					};
					device.update_with_sns_telemetry(sns);
				}
				Some(TelemetryType::State) => {
					let Ok(sts) = serde_json::from_slice(&payload) else {
						tracing::error!("error deserializing StatusSTS payload from device '{}': {payload:?}", device.id);
						return;
					};
					device.update_with_sts_telemetry(sts);
				}
				_ => {
					//
				}
			}
		}
	}

	pub async fn submit(&mut self, client: &buffered::Client) {
		for device in self.devices.iter_mut() {
			device.generate_line_protocol(client).await;
		}
	}
}

#[derive(Debug, Default)]
pub struct TasmotaDevice {
	/// Unique identifier for the tasmota device.
	id: String,

	telemetry: BTreeMap<OffsetDateTime, (ProcessedTelemetry, Option<watch::Receiver<Status>>)>,
	telemetry_buffer: BTreeMap<OffsetDateTime, MaybeTelemetry>,

	/// The most recently reported total energy usage for the device.
	reported_energy_lifetime: Option<f32>,

	/// Value to subtract from the device's reported energy usage to ensure any
	/// decrease in the supposedly monotonically-increase value gets pinned to 0,
	/// instead of some arbitrary (and likely incorrect) value.
	///
	/// This is important for when we later query these values from InfluxDB
	/// with the Flux filter '|> increase()'.
	///
	energy_lifetime_offset: f32,

	power_state: Option<PowerState>,
}

#[derive(Debug, Default)]
pub struct MaybeTelemetry {
	sns: Option<tasmota::StatusSNS>,
	sts: Option<tasmota::StatusSTS>,
}

#[derive(Debug)]
pub struct ProcessedTelemetry {
	/// Accumumlated energy usage in Watt-hours.
	pub energy: i64,
	pub power: i64,
	pub power_factor: f64,
	pub apparent_power: i64,
	pub reactive_power: i64,
	pub voltage: i64,
	pub current: f64,
	pub power_state: PowerState,
}

impl From<(tasmota::sns::Energy, tasmota::StatusSTS)> for ProcessedTelemetry {
	fn from((energy, sts): (tasmota::sns::Energy, tasmota::StatusSTS)) -> Self {
		Self {
			energy: (energy.energy_lifetime * 1000f32).round() as i64,
			power: energy.power.into(),
			power_factor: energy.power_factor as f64,
			apparent_power: energy.apparent_power.into(),
			reactive_power: energy.reactive_power.into(),
			voltage: energy.voltage.into(),
			current: energy.current.into(),
			power_state: sts.power_state,
		}
	}
}
type TelemetryPair = (tasmota::sns::Energy, tasmota::StatusSTS);

impl MaybeTelemetry {
	fn replace_sns(&mut self, sns: tasmota::StatusSNS) -> Option<tasmota::StatusSNS> {
		self.sns.replace(sns)
	}

	fn replace_sts(&mut self, sts: tasmota::StatusSTS) -> Option<tasmota::StatusSTS> {
		self.sts.replace(sts)
	}

	fn pair(&mut self) -> Option<TelemetryPair> {
		if self.sns.is_some() && self.sts.is_some() {
			let sns = self.sns.take().unwrap();
			let sts = self.sts.take().unwrap();
			Some((sns.energy, sts))
		} else {
			None
		}
	}
}

impl TasmotaDevice {
	fn update_energy_offset(&mut self, energy: f32) {
		self.energy_lifetime_offset =
			self.reported_energy_lifetime
				.map_or(energy, |previous_energy_lifetime| {
					if energy < previous_energy_lifetime {
						energy
					} else {
						// No change
						self.energy_lifetime_offset
					}
				});

		self.reported_energy_lifetime = Some(energy);
	}

	fn promote_telemetry(&mut self, timestamp: OffsetDateTime) -> Option<&ProcessedTelemetry> {
		let pair = self
			.telemetry_buffer
			.get_mut(&timestamp)
			.and_then(|v| v.pair());

		if let Some(pair) = pair {
			self.telemetry_buffer.remove(&timestamp);
			self.telemetry.insert(timestamp, (pair.into(), None));
		}

		self.telemetry.get(&timestamp).map(|(t, _)| t)
	}

	pub fn update_with_sns_telemetry(
		&mut self,
		mut sns: tasmota::StatusSNS,
	) -> Option<&ProcessedTelemetry> {
		// Compute energy offset and apply to received telemetry.
		self.update_energy_offset(sns.energy.energy_lifetime);
		sns.energy.energy_lifetime -= self.energy_lifetime_offset;

		// Append to the telemetry buffer
		let timestamp = fixup_timestamp(sns.time, OffsetDateTime::now_utc());
		self.telemetry_buffer
			.entry(timestamp)
			.or_default()
			.replace_sns(sns);

		self.promote_telemetry(timestamp)
	}

	pub fn update_with_sts_telemetry(
		&mut self,
		sts: tasmota::StatusSTS,
	) -> Option<&ProcessedTelemetry> {
		self.power_state = sts.power_state.into();

		// Append to the telemetry buffer
		let timestamp = fixup_timestamp(sts.time, OffsetDateTime::now_utc());
		self.telemetry_buffer
			.entry(timestamp)
			.or_default()
			.replace_sts(sts);

		self.promote_telemetry(timestamp)
	}

	pub fn clear_stale_buffered_telemetry(&mut self, age: Duration) -> usize {
		let threshold = OffsetDateTime::now_utc() - age;
		let before = self.telemetry_buffer.len();
		self.telemetry_buffer.retain(|&ts, _| ts >= threshold);

		self.telemetry_buffer.len() - before
	}

	pub async fn generate_line_protocol(&mut self, client: &buffered::Client) {
		for (timestamp, (telem, status)) in self
			.telemetry
			.iter_mut()
			.filter(|(_, (_, status))| status.is_none())
		{
			//
			let write_status = client
				.write_with(|builder| {
					builder
						.measurement("telemetry")
						.tag("device", &self.id)
						.field("power", telem.power)
						.timestamp(millis_from_datetime(*timestamp))
						.close_line()
				})
				.await
				.unwrap();

			*status = Some(write_status);
		}

		let submitted = self
			.telemetry
			.iter()
			.filter(|(_, (_, status))| {
				status
					.clone()
					.map(|status| *status.borrow() == Status::Accepted)
					.unwrap_or_default()
			})
			.count();

		tracing::info!("device {} has {} submitted telemetry", self.id, submitted);
	}
}

impl PartialEq for TasmotaDevice {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl PartialOrd for TasmotaDevice {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.id.cmp(&other.id).into()
	}
}

fn fixup_timestamp(value: PrimitiveDateTime, reference: OffsetDateTime) -> OffsetDateTime {
	value.assume_utc().replace_hour(reference.hour()).unwrap()
}
