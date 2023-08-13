use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PowerState {
	On,
	Off,
}

impl fmt::Display for PowerState {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{self:?}")
	}
}

impl TryFrom<&str> for PowerState {
	type Error = ();
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value.to_ascii_lowercase().as_str() {
			"on" => Ok(PowerState::On),
			"off" => Ok(PowerState::Off),
			_ => Err(()),
		}
	}
}

impl TryFrom<String> for PowerState {
	type Error = ();
	fn try_from(value: String) -> Result<Self, Self::Error> {
		match value.to_ascii_lowercase().as_str() {
			"on" => Ok(PowerState::On),
			"off" => Ok(PowerState::Off),
			_ => Err(()),
		}
	}
}
