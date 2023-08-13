use crate::{write::immediate, Precision};
use url::Url;

#[derive(Debug)]
pub struct Builder {
	client: reqwest::Client,
	host: Url,
	bucket: String,
	org_id: Option<String>,
	org_name: Option<String>,
	precision: Precision,
}

impl Builder {
	pub(crate) fn new_with(client: reqwest::Client, host: Url, bucket: String) -> Self {
		Self {
			client,
			host,
			bucket,
			org_id: Default::default(),
			org_name: Default::default(),
			precision: Default::default(),
		}
	}

	/// Set the organization name of the owner of the bucket to write.
	///
	/// This is not used for InfluxDB Cloud. Data is written to the bucket
	/// in the organization associated with the authorization token.
	pub fn org(self, name: impl Into<String>) -> Self {
		let mut s = self;
		s.org_name = Some(name.into());
		s
	}

	/// Set the organization ID of the owner of the bucket to write.
	///
	/// This is not used for InfluxDB Cloud. Data is written to the bucket
	/// in the organization associated with the authorization token.
	pub fn org_id(self, id: impl Into<String>) -> Self {
		let mut s = self;
		s.org_id = Some(id.into());
		s
	}

	pub fn precision(self, precision: Precision) -> Self {
		let mut s = self;
		s.precision = precision;
		s
	}

	pub fn build(self) -> immediate::Client {
		let client = self.client;

		// Construct the Url of the write endpoint.
		//
		let mut url = self.host;
		url.set_path("/api/v2/write");
		{
			let mut query = url.query_pairs_mut();
			query.append_pair("bucket", &self.bucket);
			query.append_pair("precision", self.precision.as_str());
			if let Some(org_name) = self.org_name {
				query.append_pair("org", &org_name);
			}
			if let Some(org_id) = self.org_id {
				query.append_pair("orgID", &org_id);
			};
		}

		immediate::Client::new(client, url)
	}
}
