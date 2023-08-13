use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusSNS {
	#[serde(rename = "Time", with = "crate::datetime")]
	pub time: PrimitiveDateTime,
	#[serde(rename = "ENERGY")]
	pub energy: Energy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Energy {
	/// Date and time from which device totals started accumulating.
	#[serde(rename = "TotalStartTime", with = "crate::datetime")]
	pub start_time: PrimitiveDateTime,
	/// Total accumulated energy used in kiloWatt hours.
	#[serde(rename = "Total")]
	pub energy_lifetime: f32,
	/// Energy used yesterday in kiloWatt hours.
	#[serde(rename = "Yesterday")]
	pub energy_yesterday: f32,
	/// Energy used today in kiloWatt hours.
	#[serde(rename = "Today")]
	pub energy_today: f32,
	/// ???
	#[serde(rename = "Period")]
	pub period: i32,
	/// Current power usage in Watts.
	#[serde(rename = "Power")]
	pub power: u32,
	/// Apparent Power in VA.
	#[serde(rename = "ApparentPower")]
	pub apparent_power: u32,
	/// Reactive Power in VAr.
	#[serde(rename = "ReactivePower")]
	pub reactive_power: u32,
	/// Power Factor.
	#[serde(rename = "Factor")]
	pub power_factor: f32,
	/// Voltage in Volts.
	#[serde(rename = "Voltage")]
	pub voltage: u32,
	/// Current in Amps.
	#[serde(rename = "Current")]
	pub current: f32,
}
