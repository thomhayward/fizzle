use crate::PowerState;
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusSTS {
	/// Clock time of the device in the local timezone.
	#[serde(rename = "Time", with = "crate::datetime")]
	pub time: PrimitiveDateTime,
	#[serde(rename = "POWER")]
	pub power: PowerState,
	/// Length of time the device has been running.
	#[serde(rename = "Uptime")]
	pub uptime: String,
	#[serde(rename = "UptimeSec")]
	pub uptime_seconds: u64,
	#[serde(rename = "Vcc")]
	pub vcc: f32,
	#[serde(rename = "LoadAvg")]
	pub load_average: u32,
	#[serde(rename = "Sleep")]
	pub sleep: u32,
	#[serde(rename = "SleepMode")]
	pub sleep_mode: String,
	#[serde(rename = "MqttCount")]
	pub mqtt_count: u32,
	#[serde(rename = "Wifi")]
	pub wifi: WiFi,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WiFi {
	/// ???
	#[serde(rename = "AP")]
	pub ap: u32,
	/// Connected SSID.
	#[serde(rename = "SSId")]
	pub ssid: String,
	#[serde(rename = "BSSId")]
	pub bssid: String,
	#[serde(rename = "Channel")]
	pub channel: u16,
	#[serde(rename = "RSSI")]
	pub rssi: i16,
	#[serde(rename = "Signal")]
	pub signal: i16,
	#[serde(rename = "LinkCount")]
	pub link_count: u16,
	#[serde(rename = "Downtime")]
	pub down_time: String,
}
