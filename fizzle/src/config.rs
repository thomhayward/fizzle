use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct Config {
	pub mqtt: MqttConfig,
	pub influxdb: InfluxConfig,
	pub display: Option<DisplayConfig>,
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
	pub host: String,
	pub port: Option<u16>,

	#[serde(default)]
	pub tls: bool,
}

#[derive(Debug, Deserialize)]
pub struct InfluxConfig {
	pub host: Url,
	pub bucket: String,
	pub token: String,
	pub org: String,
	pub read_only: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DisplayConfig {
	pub topic: String,
	#[serde(default)]
	pub retain: bool,

	pub meter_topic: String,
	pub meter_device: String,

	#[serde(default = "Vec::new")]
	pub buttons: Vec<DisplayButtonConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DisplayButtonConfig {
	pub topic: String,
	pub output_topic: String,
	pub output_payload: Option<String>,

	#[serde(default)]
	pub retain: bool,
}
