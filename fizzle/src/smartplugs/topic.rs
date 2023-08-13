#[derive(Debug)]
pub enum TelemetryType {
	Sensor,
	State,
	Lwt,
}

/// A trait for generating MQTT topics for smartplugs
///
pub trait TopicGenerator {
	/// Produce the topic string for StatusSNS telemetry messages
	fn sensor_telemetry_topic(device_name: &str) -> String;

	/// Produce the topic string for StatusSTS telemetry messages
	fn state_telemetry_topic(device_name: &str) -> String;

	/// Produce the topic string for LWT messages
	fn lwt_topic(device_name: &str) -> String;

	/// Determine the type of telemetry message from the topic string
	fn telemetry_type(topic: &str) -> Option<TelemetryType>;

	/// Extract the device name from the topic string
	fn extract_device_name(topic: &str) -> Option<&str>;
}

#[derive(Debug)]
pub struct HomeTasmotaTopicScheme;

impl TopicGenerator for HomeTasmotaTopicScheme {
	fn sensor_telemetry_topic(device_name: &str) -> String {
		format!("tasmota/tele/{}/SENSOR", device_name)
	}

	fn state_telemetry_topic(device_name: &str) -> String {
		format!("tasmota/tele/{}/STATE", device_name)
	}

	fn lwt_topic(device_name: &str) -> String {
		format!("tasmota/tele/{}/LWT", device_name)
	}

	fn telemetry_type(topic: &str) -> Option<TelemetryType> {
		if topic.ends_with("/SENSOR") {
			Some(TelemetryType::Sensor)
		} else if topic.ends_with("/STATE") {
			Some(TelemetryType::State)
		} else if topic.ends_with("/LWT") {
			Some(TelemetryType::Lwt)
		} else {
			None
		}
	}

	fn extract_device_name(topic: &str) -> Option<&str> {
		let topic = topic.trim_start_matches("tasmota/tele/");
		let topic = topic.trim_end_matches("/SENSOR");
		let topic = topic.trim_end_matches("/STATE");
		let topic = topic.trim_end_matches("/LWT");
		Some(topic)
	}
}

#[cfg(test)]
mod tests {
	use crate::smartplugs::{topic::HomeTasmotaTopicScheme, SmartPlug};

	#[test]
	fn test_smartplug_new() {
		let name = "test".to_string();
		let smartplug = SmartPlug::<HomeTasmotaTopicScheme>::new(name.clone());
		assert_eq!(smartplug.name(), name);
	}

	#[test]
	fn test_smartplug_sensor_telemetry_topic() {
		let name = "location/device-name".to_string();
		let smartplug = SmartPlug::<HomeTasmotaTopicScheme>::new(name.clone());
		assert_eq!(
			smartplug.sensor_telemetry_topic(),
			"tasmota/tele/location/device-name/SENSOR"
		);
	}

	#[test]
	fn test_smartplug_state_telemetry_topic() {
		let name = "location/device-name".to_string();
		let smartplug = SmartPlug::<HomeTasmotaTopicScheme>::new(name.clone());
		assert_eq!(
			smartplug.state_telemetry_topic(),
			"tasmota/tele/location/device-name/STATE"
		);
	}

	#[test]
	fn test_smartplug_lwt_topic() {
		let name = "location/device-name".to_string();
		let smartplug = SmartPlug::<HomeTasmotaTopicScheme>::new(name.clone());
		assert_eq!(
			smartplug.lwt_topic(),
			"tasmota/tele/location/device-name/LWT"
		);
	}
}
