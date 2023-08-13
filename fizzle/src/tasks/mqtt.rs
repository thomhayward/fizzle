use rumqttc::{AsyncClient, Event, EventLoop, Incoming, Publish, QoS, SubscribeFilter};
use tokio::sync::{mpsc, watch};
use url::Url;

pub struct Channels {
	pub impulse_raw_tx: mpsc::Sender<Publish>,
	pub impulse_tx: mpsc::Sender<Publish>,
	pub tasmota_tx: mpsc::Sender<Publish>,
}

/// Ensure that the MQTT URL has a client_id query parameter.
///
pub fn force_mqtt_client(u: &str, default_client: &str) -> Result<Url, url::ParseError> {
	let mut mqtt_url = Url::parse(u)?;
	let mut mqtt_query = mqtt_url.query_pairs();
	if !mqtt_query.any(|(key, _)| key == "client_id") {
		tracing::warn!("'client_id' not specified, using {default_client}");
		mqtt_url
			.query_pairs_mut()
			.append_pair("client_id", default_client);
	}
	Ok(mqtt_url)
}

pub async fn start_task(
	client: AsyncClient,
	mut event_loop: EventLoop,
	channels: Channels,
	mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
	let Channels {
		impulse_raw_tx,
		impulse_tx,
		tasmota_tx,
	} = channels;
	let mut should_shutdown = false;

	loop {
		tokio::select! {
		  event = event_loop.poll() => {
		match event {
		  Ok(Event::Incoming(Incoming::ConnAck(_))) => {
			tracing::debug!("connected to mqtt broker, subscribing to topics");
			client.subscribe_many([
			  SubscribeFilter::new("meter-reader/impulse/raw".into(), QoS::ExactlyOnce),
			  SubscribeFilter::new("meter-reader/impulse".into(), QoS::ExactlyOnce),
			  SubscribeFilter::new("tasmota/tele/#".into(), QoS::ExactlyOnce)]
			)
			.await?;
		  }
		  Ok(Event::Incoming(Incoming::Publish(message))) => {
			let topic = &message.topic;
			if topic == "meter-reader/impulse/raw" {
			  impulse_raw_tx.send(message).await?;
			  continue;
			}
			if topic == "meter-reader/impulse" {
			  impulse_tx.send(message).await?;
			  continue;
			}

			if topic.starts_with("tasmota/tele/") {
			  tasmota_tx.send(message).await?;
			}
		  }
		  Ok(event) => {
			tracing::debug!("mqtt event: {event:?}");
		  }
		  Err(error) => {
			tracing::error!("mqtt error: {error:?}");
			if should_shutdown {
			  break;
			}
		  }
		}
		  }
		  _ = shutdown.changed() => {
		tracing::info!("shutting down mqtt task");
		client.disconnect().await?;
		should_shutdown = true;
		  }
		}
	}
	Ok(())
}
