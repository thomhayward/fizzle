use crate::config::{Config, DisplayButtonConfig, DisplayConfig};
use fizzle::util::parse_json_payload;
use influxdb::query::QueryClient;
use mqtt::{clients::tokio::Client, QoS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::{Date, Duration, OffsetDateTime};
use tokio::{
	sync::{watch, RwLock},
	task::JoinHandle,
};
use yesterday::Record;

#[derive(Debug, Deserialize)]
pub struct MeterReading {
	pub power: u16,
	pub energy_today: u32,
	pub energy_yesterday: u32,
	pub energy_lifetime: u64,
}

#[derive(Debug, Serialize)]
struct Page {
	lines: Vec<String>,
}

pub fn create_task<'c>(
	client: Client,
	query_client: QueryClient,
	config: Arc<Config>,
	shutdown: watch::Receiver<bool>,
) -> JoinHandle<anyhow::Result<()>> {
	tokio::spawn(start_task(client, query_client, config, shutdown))
}

pub async fn start_task(
	mqtt_client: Client,
	query_client: QueryClient,
	config: Arc<Config>,
	mut shutdown_signal: watch::Receiver<bool>,
) -> anyhow::Result<()> {
	let Some(display_config) = config.display.clone() else {
		tracing::error!("no display configuration. skipping character display task");
		return Ok(());
	};

	tokio::spawn(button_task(mqtt_client.clone(), display_config.clone()));
	let mut impulses = mqtt_client.subscribe(display_config.meter_topic, 8).await?;

	let yesterdays_data: Arc<RwLock<Option<(Date, Vec<Record>)>>> = Default::default();
	tokio::spawn(data_update_task(
		query_client,
		config,
		Arc::clone(&yesterdays_data),
		shutdown_signal.clone(),
	));

	loop {
		#[rustfmt::skip]
		let message = tokio::select! {
		  Some(message) = impulses.recv() => message,
		  _ = shutdown_signal.changed() => {
				tracing::info!("shutting down character display task");
				mqtt_client.publish(
					display_config.topic.as_str(),
					"\n  meter  agent\n    shutdown\n ",
					QoS::AtMostOnce,
					display_config.retain
				).await?;
				break;
		  }
		};

		let Ok(payload): Result<MeterReading, _> = parse_json_payload(message) else {
			continue;
		};

		tracing::debug!("received impulse: {payload:?}");

		let now = OffsetDateTime::now_local().expect("WTF!");

		let yesterday_usage = if let Some((date, data)) = yesterdays_data.read().await.as_ref() {
			let yesterday = now.checked_sub(Duration::days(1)).unwrap();
			if &yesterday.date() == date {
				data.iter()
					.find(|Record { ts, .. }| ts >= &yesterday)
					.cloned()
			} else {
				None
			}
		} else {
			None
		};

		let line3 = if let Some(yesterday_usage) = yesterday_usage {
			let Record { ts, value } = yesterday_usage;
			format!(
				"Yn{: >5}Wh @{: >4.0}W",
				value,
				(value as f64 * 3600.0
					/ (ts.hour() as u32 * 3600 + ts.minute() as u32 * 60 + ts.second() as u32)
						as f64)
					.round()
			)
		} else {
			String::default()
		};

		let page = format!(
			"{:02}:{:02}:{:02} {: >6}W\nT {: >5}Wh @{: >4.0}W\n{line3}\nYt{: >5}Wh @{: >4.0}W",
			now.hour(),
			now.minute(),
			now.second(),
			payload.power,
			payload.energy_today,
			(payload.energy_today as f64 * 3600.0
				/ (now.hour() as u32 * 3600 + now.minute() as u32 * 60 + now.second() as u32)
					as f64)
				.round(),
			payload.energy_yesterday,
			(payload.energy_yesterday as f64 * 3600.0 / 86400.0).round()
		);

		tracing::debug!("generated page: {page:?}");
		mqtt_client
			.publish(
				display_config.topic.as_str(),
				page,
				QoS::AtMostOnce,
				display_config.retain,
			)
			.await?;
	}
	Ok(())
}

async fn fetch_yesterdays_energy_data(
	query_client: QueryClient,
	config: Arc<Config>,
	yesterdays_data: Arc<RwLock<Option<(Date, Vec<Record>)>>>,
) {
	let date = OffsetDateTime::now_local()
		.unwrap()
		.date()
		.previous_day()
		.unwrap();

	tracing::info!("fetching {date}'s energy usage data");

	// Fetch yesterdays's energy usage data.
	if let Ok(data) = yesterday::fetch(
		&query_client,
		date,
		&config.influxdb.bucket,
		&config.display.as_ref().unwrap().meter_device,
	)
	.await
	{
		yesterdays_data.write().await.replace((date, data));
	}
}

async fn data_update_task(
	query_client: QueryClient,
	config: Arc<Config>,
	yesterdays_data: Arc<RwLock<Option<(Date, Vec<Record>)>>>,
	mut shutdown_signal: watch::Receiver<bool>,
) -> anyhow::Result<()> {
	let mut check_interval = tokio::time::interval(std::time::Duration::from_secs(600));

	loop {
		tokio::select! {
			_ = check_interval.tick() => {},
			_ = shutdown_signal.changed() => break,
		}

		// Determine if we need to fetch yesterday's data.
		let needs_update = if let Some((date, _)) = *yesterdays_data.read().await {
			let yesterday = OffsetDateTime::now_local()
				.unwrap()
				.date()
				.previous_day()
				.unwrap();

			date < yesterday
		} else {
			true
		};

		if needs_update {
			fetch_yesterdays_energy_data(
				query_client.clone(),
				Arc::clone(&config),
				Arc::clone(&yesterdays_data),
			)
			.await;
		}
	}

	Ok(())
}

async fn button_task(mqtt_client: Client, display_config: DisplayConfig) -> anyhow::Result<()> {
	if display_config.buttons.is_empty() {
		return Ok(());
	}

	let button_topics: Vec<_> = display_config
		.buttons
		.iter()
		.map(|DisplayButtonConfig { topic, .. }| topic.as_str())
		.collect();

	// Subscribe to the button topics.
	let mut buttons = mqtt_client
		.subscribe(button_topics.as_slice(), button_topics.len())
		.await?;

	while let Some(message) = buttons.recv().await {
		// Find the button configuration for the received message.
		let Some(button_config) = display_config
			.buttons
			.iter()
			.find(|DisplayButtonConfig { topic, .. }| topic == message.topic.as_str())
		else {
			continue;
		};

		// If the user supplied a payload in the configuration file, use that as
		// the payload for the outgoing message. Otherwise use the payload from
		// the incoming message.
		//
		let payload = match &button_config.output_payload {
			Some(payload) => payload.as_bytes().to_vec(),
			None => message.payload.to_vec(),
		};

		mqtt_client
			.publish(
				button_config.output_topic.as_str(),
				payload,
				QoS::AtMostOnce,
				button_config.retain,
			)
			.await?;
	}
	Ok(())
}
