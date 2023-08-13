use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct Config<'a> {
	pub mqtt: Url,

	#[serde(borrow = "'a")]
	pub influxdb: Option<InfluxConfig<'a>>,

	pub display_topic: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct InfluxConfig<'a> {
	pub host: Url,
	pub bucket: &'a str,
	pub token: &'a str,
	pub org: Option<&'a str>,
	pub org_id: Option<&'a str>,
}
