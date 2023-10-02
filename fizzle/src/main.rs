use clap::Parser;
use config::Config;
use fizzle::{
	impulse::{Impulse, ImpulseContext},
	smartplugs::{topic::HomeTasmotaTopicScheme, SmartPlugSwarm},
	util::{parse_json_payload, timestamp_ms},
};
use influxdb::util::stdout_buffered_client;
use mqtt::clients::tokio::{tcp_client, Options};
use std::{fs::File, io::Read, path::PathBuf};
use time::util::local_offset::Soundness;
use tokio::sync::watch;

mod config;
mod tasks;

#[derive(Parser)]
pub struct Arguments {
	#[clap(env = "FIZZLE_CONFIG_PATH")]
	config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	unsafe {
		// Hypothetical unsoundness be damned!
		//
		// SAFETY: We do not modify our own environment so this _might_ be OK?.
		time::util::local_offset::set_soundness(Soundness::Unsound);
	}

	let arguments = Arguments::parse();

	// Read the configuration
	let mut config_file = File::open(arguments.config)?;
	let mut config = String::new();
	config_file.read_to_string(&mut config)?;
	let config: Config = serde_yaml::from_str(&config)?;

	let (writer, influxdb_task) = if let Some(influxdb_config) = config.influxdb {
		let client = influxdb::Client::new(influxdb_config.host, influxdb_config.token)?;
		let mut write_builder = client
			.write_to_bucket(influxdb_config.bucket)
			.precision(influxdb::Precision::Milliseconds);
		if let Some(org_id) = influxdb_config.org_id {
			write_builder = write_builder.org(org_id);
		}
		if let Some(org) = influxdb_config.org {
			write_builder = write_builder.org(org);
		}
		write_builder.build().buffered()
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

	let mut impulse_context: Option<ImpulseContext> = None;

	let (shutdown_tx, shutdown_rx) = watch::channel(false);

	// Spawn a task to handle incoming MQTT messages
	//
	let options = Options {
		host: "mqtt.tjh.dev".into(),
		tls: false,
		..Default::default()
	};
	let (client, handle) = tcp_client(options);
	let mut impulse_raw_rx = client.subscribe("meter-reader/impulse/raw", 64).await?;
	let mut tasmota_rx = client.subscribe("tasmota/tele/#", 64).await?;

	// Spawn a task to drive the character display device
	//
	let display_task = tasks::display::create_task(
		client.clone(),
		config.display_topic.map(String::from),
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

	influxdb_task.await??;
	display_task.await??;

	client.disconnect().await?;
	let _ = handle.await?;

	Ok(())
}
