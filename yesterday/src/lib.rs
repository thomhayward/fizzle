use influxdb::query::QueryClient;
use serde::Deserialize;
use time::{
	format_description::well_known::Rfc3339,
	macros::{offset, time},
	Date, OffsetDateTime,
};

const QUERY: &str = r#"
	from(bucket: "params.bucket")
	  |> range(start: params.dayStart, stop: params.dayStop)
	  |> filter(fn: (r) => r["_measurement"] == "impulse")
	  |> filter(fn: (r) => r["_field"] == "energy")
	  |> filter(fn: (r) => r["device"] == "params.device")
	  |> increase()
	  |> aggregateWindow(every: 1m, fn: last, createEmpty: false)
	  |> yield(name: "mean")
"#;

pub async fn fetch(
	client: &QueryClient,
	date: Date,
	bucket: &str,
	device: &str,
) -> anyhow::Result<Vec<Record>> {
	//
	let offset = OffsetDateTime::now_local().unwrap().offset();
	let start = date.with_time(time!(00:00:00)).assume_offset(offset);
	let end = date
		.next_day()
		.unwrap()
		.with_time(time!(00:00:00))
		.assume_offset(offset);

	let response = client
		.query(
			QUERY,
			[
				("bucket", bucket),
				("device", device),
				(
					"dayStart",
					start.to_offset(offset!(+0)).format(&Rfc3339)?.as_str(),
				),
				(
					"dayStop",
					end.to_offset(offset!(+0)).format(&Rfc3339)?.as_str(),
				),
			],
		)
		.await?;

	let data = if response.status().is_success() {
		response.text().await?
	} else {
		panic!("{:?}", response.text().await?);
	};

	let mut result = Vec::new();
	let mut rdr = csv::ReaderBuilder::new()
		.has_headers(true)
		.comment(Some(b'#'))
		.from_reader(data.as_bytes());
	for res in rdr.deserialize() {
		let rec: Record = res?;
		result.push(rec);
	}

	Ok(result)
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Record {
	#[serde(rename = "_time", with = "time::serde::rfc3339")]
	pub ts: OffsetDateTime,

	#[serde(rename = "_value")]
	pub value: u32,
}
