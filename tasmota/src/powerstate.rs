use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct UnknownPowerStateLiteral<'a>(Cow<'a, str>);

impl<'a> fmt::Display for UnknownPowerStateLiteral<'a> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let Self(literal) = self;
		write!(f, "Unknown literal for PowerState: {literal}")
	}
}

impl<'a> TryFrom<&'a str> for PowerState {
	type Error = UnknownPowerStateLiteral<'a>;
	fn try_from(value: &'a str) -> Result<Self, Self::Error> {
		match value.to_ascii_lowercase().as_str() {
			"on" => Ok(PowerState::On),
			"off" => Ok(PowerState::Off),
			_ => Err(UnknownPowerStateLiteral(Cow::Borrowed(value))),
		}
	}
}

impl TryFrom<String> for PowerState {
	type Error = UnknownPowerStateLiteral<'static>;
	fn try_from(value: String) -> Result<Self, Self::Error> {
		match value.to_ascii_lowercase().as_str() {
			"on" => Ok(PowerState::On),
			"off" => Ok(PowerState::Off),
			_ => Err(UnknownPowerStateLiteral(Cow::Owned(value))),
		}
	}
}

impl From<bool> for PowerState {
	fn from(value: bool) -> Self {
		match value {
			true => Self::On,
			false => Self::Off,
		}
	}
}
