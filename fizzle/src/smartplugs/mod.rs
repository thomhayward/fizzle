mod smartplug;
pub mod topic;

use self::topic::{TelemetryType, TopicGenerator};
use crate::util::{bytes_to_string, parse_json_payload};
use influxdb::buffered;
pub use smartplug::SmartPlug;
use std::{collections::BTreeMap, error, fmt};
use tasmota::{sns::StatusSNS, PowerState, StatusSTS};

#[derive(Debug)]
pub struct SmartPlugSwarm<G: TopicGenerator> {
	writer: buffered::Client,
	smartplugs: BTreeMap<String, SmartPlug<G>>,
	telemetry_map: BTreeMap<String, String>,
}

impl<G: TopicGenerator + fmt::Debug> SmartPlugSwarm<G> {
	pub fn new(writer: buffered::Client) -> Self {
		Self {
			writer,
			smartplugs: BTreeMap::new(),
			telemetry_map: BTreeMap::new(),
		}
	}

	pub fn create_new_smartplug(&mut self, name: String) -> Option<SmartPlug<G>> {
		let smartplug = SmartPlug::new(name);

		// Remove any existing smartplug with the same name.
		let existing_smartplug = self.smartplugs.get(smartplug.name());
		if let Some(existing_smartplug) = existing_smartplug {
			self.telemetry_map
				.remove(&existing_smartplug.sensor_telemetry_topic());
			self.telemetry_map
				.remove(&existing_smartplug.state_telemetry_topic());
			self.telemetry_map.remove(&existing_smartplug.lwt_topic());
		}

		self.telemetry_map.insert(
			smartplug.sensor_telemetry_topic(),
			smartplug.name().to_string(),
		);
		self.telemetry_map.insert(
			smartplug.state_telemetry_topic(),
			smartplug.name().to_string(),
		);
		self.telemetry_map
			.insert(smartplug.lwt_topic(), smartplug.name().to_string());

		self.smartplugs
			.insert(smartplug.name().to_string(), smartplug)
	}

	pub async fn handle_telemetry(
		&mut self,
		message: rumqttc::Publish,
	) -> Result<(), Box<dyn error::Error + 'static>> {
		//
		let topic = &message.topic.clone();
		let mut smartplug_name = self.telemetry_map.get(topic).map(|s| s.as_str());
		if smartplug_name.is_none() {
			tracing::warn!("handling telemetry from unknown topic: {topic}");
			if let Some(name) = G::extract_device_name(topic) {
				tracing::warn!("extracted device name: {name}");
				smartplug_name = Some(name);
				if let Some(old_plug) = self.create_new_smartplug(name.to_string()) {
					tracing::warn!(
						"created new smartplug: {name}, replacing old smartplug: {old_plug:?}"
					);
				}
			}
		}

		let Some(smartplug_name) = smartplug_name else {
      tracing::error!("received telemetry for unknown topic: {}", topic);
      return Err("unknown topic".into());
    };

		let Some(smartplug) = self.smartplugs.get_mut(smartplug_name) else {
      tracing::error!("received telemetry for unknown smartplug: {}", smartplug_name);
      return Ok(());
    };

		match G::telemetry_type(topic) {
			Some(TelemetryType::Sensor) => {
				let telemetry = parse_json_payload::<StatusSNS>(message)?;
				smartplug.append_sensor_telemetry(telemetry);
			}
			Some(TelemetryType::State) => {
				let telemetry = parse_json_payload::<StatusSTS>(message)?;
				smartplug.append_state_telemetry(telemetry);
			}
			Some(TelemetryType::Lwt) => {
				// The Tasmota LWT payload is just a string.
				let lwt = bytes_to_string(message.payload)?;
				smartplug.set_lwt(lwt);
			}
			None => {
				tracing::warn!("unknown telemetry type received for device '{smartplug_name}' on topic '{topic}'");
			}
		}

		if let Some((dt, sns, sts)) = smartplug.matched_telemetry() {
			//
			let telemetry = smartplug.generate_telemetry(dt, sns, sts)?;
			self.writer
				.write_with(|builder| {
					builder
						.measurement("telemetry")
						.tag("device", &telemetry.name)
						.field("apparent_power", telemetry.apparent_power)
						.field("current", telemetry.current)
						.field("device_uptime", telemetry.device_uptime)
						.field("energy", telemetry.energy)
						.field("monitor_uptime", telemetry.monitor_uptime)
						.field("power", telemetry.power)
						.field("power_factor", telemetry.power_factor)
						.field("reactive_power", telemetry.reactive_power)
						.field(
							"state",
							match telemetry.state {
								PowerState::On => "on",
								PowerState::Off => "off",
							},
						)
						.field("voltage", telemetry.voltage)
						.timestamp(telemetry.timestamp)
						.close_line()
				})
				.await?;
		}

		Ok(())
	}
}
