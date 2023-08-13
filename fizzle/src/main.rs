use clap::Parser;
use fizzle::{
	impulse::{Impulse, ImpulseContext},
	smartplugs::{topic::HomeTasmotaTopicScheme, SmartPlugSwarm},
	util::{parse_json_payload, timestamp_ms},
};
use influxdb::util::stdout_buffered_client;
use rumqttc::Publish;
use std::error;
use time::util::local_offset::Soundness;
use tokio::sync::{mpsc, watch};

mod tasks;

#[derive(Parser)]
pub struct Arguments {
	#[clap(short, long, env = "MQTT_URL")]
	mqtt: String,

	#[clap(long, env = "FIZZLE_DISPLAY_TOPIC")]
	display_topic: Option<String>,

	#[clap(long, env = "INFLUXDB_HOST")]
	influxdb_host: Option<String>,

	#[clap(long, env = "INFLUXDB_ORG")]
	influxdb_org: Option<String>,

	#[clap(long, env = "INFLUXDB_BUCKET")]
	influxdb_bucket: Option<String>,

	#[clap(long, env = "INFLUXDB_TOKEN")]
	influxdb_token: Option<String>,

	#[clap(long, default_value = "60")]
	influxdb_interval: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error + 'static>> {
	tracing_subscriber::fmt::init();

	unsafe {
		// Hypothetical unsoundness be damned!
		//
		// SAFETY: We do not modify our own environment so this _might_ be OK?.
		time::util::local_offset::set_soundness(Soundness::Unsound);
	}

	// Load environment variables from .env file if this is a debug build.
	#[cfg(debug_assertions)]
	if dotenvy::dotenv().is_err() {
		tracing::warn!("could not load '.env' file");
	};

	let arguments = Arguments::parse();

	let (writer, influxdb_task) = if let (Some(host), Some(token), Some(bucket), Some(org)) = (
		&arguments.influxdb_host,
		&arguments.influxdb_token,
		&arguments.influxdb_bucket,
		&arguments.influxdb_org,
	) {
		let client = influxdb::Client::new(host, token)?;
		client
			.write_to_bucket(bucket)
			.org(org)
			.precision(influxdb::Precision::Milliseconds)
			.build()
			.buffered()
	} else {
		stdout_buffered_client()
	};

	writer
		.write_with(|builder| {
			builder
				.measurement("fizzle")
				.tag("reason", "started")
				.field("pid", std::process::id() as u64)
				.close_line()
		})
		.await?;

	let mqtt_options = rumqttc::MqttOptions::parse_url(
		tasks::mqtt::force_mqtt_client(&arguments.mqtt, "fizzle")?.as_str(),
	)?;

	let mut impulse_context: Option<ImpulseContext> = None;

	let (shutdown_tx, shutdown_rx) = watch::channel(false);
	let (impulse_raw_tx, mut impulse_raw_rx) = mpsc::channel::<Publish>(64);
	let (impulse_tx, impulse_rx) = mpsc::channel::<Publish>(64);
	let (tasmota_tx, mut tasmota_rx) = mpsc::channel::<Publish>(64);

	// Spawn a task to handle incoming MQTT messages
	//
	let (client, event_loop) = rumqttc::AsyncClient::new(mqtt_options, 128);
	let mqtt_task = tokio::spawn(tasks::mqtt::start_task(
		client.clone(),
		event_loop,
		tasks::mqtt::Channels {
			impulse_raw_tx,
			impulse_tx,
			tasmota_tx,
		},
		shutdown_rx.clone(),
	));

	// Spawn a task to drive the character display device
	//
	tasks::display::create_task(
		client.clone(),
		arguments.display_topic,
		impulse_rx,
		shutdown_rx.clone(),
	);

	// Create the smart plug swarm!
	let mut swarm: SmartPlugSwarm<HomeTasmotaTopicScheme> = SmartPlugSwarm::new(writer.clone());

	loop {
		tokio::select! {
			// "Smart" Meter Impulse Messages
			Some(message) = impulse_raw_rx.recv() => {
				// Parse the payload as an Impulse object.
				let payload: Impulse = match parse_json_payload(message) {
					Ok(payload) => payload,
					Err(error) => {
						tracing::error!("error parsing impulse payload: {error:?}");
						continue;
					}
				};

				let context = impulse_context
					.get_or_insert_with(||
						ImpulseContext::with_initial_count(payload.impulse_count as i64)
					);

				if (payload.impulse_count as i64) < context.previous_count {
					tracing::info!("impulse counter reset detected, adjusting offset");
					context.offset = context.previous_count;
				}


					writer
						.write_with(context.write_line_protocol_with(&payload, &timestamp_ms()))
						.await?;


				// Update the count
				context.previous_count = payload.impulse_count.into();
			}
			Some(message) = tasmota_rx.recv() => {
				let Err(error) = swarm.handle_telemetry(message).await else {
					continue
				};
				tracing::error!("error handling telemetry: {error:?}");
			}
			_ = tokio::signal::ctrl_c() => {
				tracing::debug!("received ctrl-c, closing");
				shutdown_tx.send(true)?;
				break
			},
		}
	}

	drop(swarm);
	drop(writer);

	mqtt_task.await??;
	influxdb_task.await??;

	Ok(())
}
