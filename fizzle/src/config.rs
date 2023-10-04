use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct Config {
	pub mqtt: MqttConfig,
	pub influxdb: InfluxConfig,
	pub display_topic: Option<String>,
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
