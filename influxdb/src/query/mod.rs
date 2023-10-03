use std::{borrow::Cow, collections::BTreeMap, str::from_utf8};

use reqwest::{
	header::{HeaderValue, ACCEPT, CONTENT_TYPE},
	Method, Response, Url,
};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Serialize)]
struct QueryPayload<'a> {
	#[serde(borrow)]
	dialect: Option<Dialect<'a>>,

	#[serde(with = "time::serde::rfc3339")]
	now: OffsetDateTime,

	#[serde(borrow)]
	query: &'a str,

	#[serde(borrow, rename = "type")]
	ty: &'a str,
	// #[serde(borrow)]
	// params: BTreeMap<&'a str, &'a str>,
}

#[derive(Serialize)]
struct Dialect<'a> {
	#[serde(borrow)]
	annotations: &'a [&'a str],

	header: bool,
}

#[derive(Clone, Debug)]
pub struct QueryClient {
	pub(crate) client: reqwest::Client,
	pub(crate) url: Url,
}

impl QueryClient {
	pub fn org<T: AsRef<str>>(mut self, name: T) -> Self {
		self.url.query_pairs_mut().append_pair("org", name.as_ref());
		self
	}

	pub fn org_id<T: AsRef<str>>(mut self, id: T) -> Self {
		self.url
			.query_pairs_mut()
			.append_pair("org_id", id.as_ref());
		self
	}

	pub async fn query<'a, T: AsRef<str>, P: Into<BTreeMap<&'a str, &'a str>>>(
		&self,
		flux: T,
		params: P,
	) -> anyhow::Result<Response> {
		//
		let mut query = Cow::Borrowed(flux.as_ref());
		for (k, v) in params.into().into_iter() {
			let search = format!("params.{k}");
			query = Cow::Owned(query.replace(&search, v));
		}

		let payload = QueryPayload {
			dialect: Some(Dialect {
				annotations: &["datatype", "default", "group"],
				header: true,
			}),
			now: OffsetDateTime::now_utc(),
			query: &query,
			ty: "flux",
		};

		let body = serde_json::to_vec(&payload)?;
		eprintln!("{}", from_utf8(&body)?);

		let response = self
			.client
			.request(Method::POST, self.url.clone())
			.header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
			.header(ACCEPT, HeaderValue::from_static("application/csv"))
			.body(body)
			.send()
			.await?;

		Ok(response)
	}
}
