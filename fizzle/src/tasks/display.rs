use fizzle::util::parse_json_payload;
use rumqttc::{AsyncClient, Publish, QoS};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::{
	sync::{mpsc, watch},
	task::JoinHandle,
};

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
	mqtt_client: AsyncClient,
	topic: Option<String>,
	mut impulse_rx: mpsc::Receiver<Publish>,
	shutdown: watch::Receiver<bool>,
) -> JoinHandle<anyhow::Result<()>> {
	if let Some(topic) = topic {
		tracing::info!("starting character display task");
		tokio::spawn(start_task(mqtt_client, topic, impulse_rx, shutdown))
	} else {
		tracing::info!("starting dummy character display task");
		tokio::spawn(async move {
			while impulse_rx.recv().await.is_some() {
				//
			}
			Ok(())
		})
	}
}

pub async fn start_task(
	client: AsyncClient,
	topic: String,
	mut impulses: mpsc::Receiver<Publish>,
	mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
	loop {
		tokio::select! {
		  Some(message) = impulses.recv() => {
					let Ok(payload): Result<MeterReading, _> = parse_json_payload(message) else {
						continue;
					};

					tracing::debug!("received impulse: {payload:?}");

					let now = OffsetDateTime::now_local().expect("WTF!");
					let page = Page {
						lines: vec![
							format!(
								"{:02}:{:02}:{:02} {: >6}W",
								now.hour(),
								now.minute(),
								now.second(),
								payload.power
							),
							format!(
								"T {: >5}Wh @{: >4.0}W",
								payload.energy_today,
								(payload.energy_today as f64 * 3600.0 / (now.hour() as u32 * 3600 + now.minute() as u32 * 60 + now.second() as u32) as f64).round()
							),
							format!("                "),
							format!(
								"Yt{: >5}Wh @{: >4.0}W",
								payload.energy_yesterday,
								(payload.energy_yesterday as f64 * 3600.0 / 86400.0).round()
							)
						],
					};

					tracing::debug!("generated page: {page:?}");

					client.publish(
						&topic,
						QoS::ExactlyOnce,
						false,
						serde_json::to_vec(&page)?
					).await?;

					tracing::info!("published meter display page to topic '{topic}'");
				}
		  _ = shutdown.changed() => {
		tracing::info!("shutting down display task");
		break;
		  }
		}
	}
	Ok(())
}
