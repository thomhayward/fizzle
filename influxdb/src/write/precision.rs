#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precision {
	Nanoseconds,
	Microseconds,
	Milliseconds,
	Seconds,
}

impl Precision {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Nanoseconds => "ns",
			Self::Microseconds => "us",
			Self::Milliseconds => "ms",
			Self::Seconds => "s",
		}
	}
}

impl ToString for Precision {
	fn to_string(&self) -> String {
		self.as_str().into()
	}
}

impl Default for Precision {
	fn default() -> Self {
		Self::Nanoseconds
	}
}

#[cfg(test)]
mod tests {
	use super::Precision;

	#[test]
	fn ordering() {
		assert!(Precision::Nanoseconds < Precision::Microseconds);
		assert!(Precision::Microseconds < Precision::Milliseconds);
		assert!(Precision::Milliseconds < Precision::Seconds);
	}
}
