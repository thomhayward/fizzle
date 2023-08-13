mod powerstate;
pub use powerstate::PowerState;

// Sensor telemetry messages
//
pub mod sns;
pub use sns::StatusSNS;

mod status0;

// Status telemetry messages
//
pub mod sts;
pub use sts::StatusSTS;

use time::format_description::FormatItem;

/// Date-string format used by Tasmota-based devices.
pub const DATETIME_FORMAT: &[FormatItem<'_>] =
	time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");

// Generate a format module to deserialize/serialize PrimitiveDateTime's with
// serde annotations.
//
// #[derive(Debug, Deserialize, Serialize)]
// struct Example {
// 	 #[serde(with = "tasmota::datetime")]
//	 time: PrimitiveDateTime,
// }
//
time::serde::format_description!(datetime, PrimitiveDateTime, DATETIME_FORMAT);
