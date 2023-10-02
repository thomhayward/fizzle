use fizzle::util::parse_json_payload;
use mqtt::{clients::tokio::Client, QoS};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::{sync::watch, task::JoinHandle};

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

// const TOPIC: &str = "fizzle/meter-display/page";

pub fn create_task(
	client: Client,
	topic: Option<String>,
	shutdown: watch::Receiver<bool>,
) -> JoinHandle<anyhow::Result<()>> {
	if let Some(topic) = topic {
		tracing::info!("starting character display task");
		tokio::spawn(start_task(client, topic, shutdown))
	} else {
		tracing::info!("starting dummy character display task");
		tokio::spawn(async move {
			let mut impulses = client.subscribe("meter-reader/impulse", 64).await?;
			while impulses.recv().await.is_some() {
				//
			}
			Ok(())
		})
	}
}

pub async fn start_task(
	client: Client,
	topic: String,
	mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
	let mut impulses = client.subscribe("meter-reader/impulse", 64).await?;
	let mut buttons = client.subscribe("fizzle/meter-display/button/+", 1).await?;

	loop {
		#[rustfmt::skip]
		tokio::select! {
		  Some(message) = impulses.recv() => {
				let Ok(payload): Result<MeterReading, _> = parse_json_payload(message) else {
					continue;
				};

				tracing::debug!("received impulse: {payload:?}");

				let now = OffsetDateTime::now_local().expect("WTF!");
				let page =
						format!(
							"{:02}:{:02}:{:02} {: >6}W\nT {: >5}Wh @{: >4.0}W\n\nYt{: >5}Wh @{: >4.0}W",
							now.hour(),
							now.minute(),
							now.second(),
							payload.power,
							payload.energy_today,
							(payload.energy_today as f64 * 3600.0 / (now.hour() as u32 * 3600 + now.minute() as u32 * 60 + now.second() as u32) as f64).round(),
							payload.energy_yesterday,
							(payload.energy_yesterday as f64 * 3600.0 / 86400.0).round()
						);

				tracing::debug!("generated page: {page:?}");
				client.publish(topic.as_str(), page, QoS::AtMostOnce, true).await?;

				// tracing::info!("published meter display page to topic '{topic}'");
			}
			Some(message) = buttons.recv() => {
				// Determine which button was pressed.
				//
				// The button name is the last level of the button topic.
				let Some(name) = message.topic.levels().last() else {
					continue
				};

				// Match the button to the smart-plug topic.
				let switch_topic = match name {
					"A" => "tasmota/cmnd/lounge/light2/Power0",
					"B" => "tasmota/cmnd/lounge/light/Power0",
					name => {
						tracing::warn!("unknown button '{name}'");
						continue;
					}
				};

				// Toggle the power for the smart-plug.
				client.publish(switch_topic, "toggle", QoS::AtMostOnce, false).await?;
			}
		  _ = shutdown.changed() => {
				tracing::info!("shutting down display task");
				client.publish(
					topic.as_str(),
					"\n  meter  agent\n    shutdown\n ",
					QoS::AtMostOnce,
					true
				).await?;
				break;
		  }
		}
	}
	Ok(())
}
