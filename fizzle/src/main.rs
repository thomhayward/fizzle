mod config;
mod tasks;

use clap::Parser;
use config::Config;
use fizzle::smartplugs::{topic::HomeTasmotaTopicScheme, SmartPlugSwarm};
use influxdb::{util::stdout_buffered_client, Client as InfluxDbClient, Precision};
use mqtt::{
	clients::tokio::{tcp_client, Options},
	FilterBuf,
};
use std::{
	fs::File,
	path::{Path, PathBuf},
	sync::Arc,
};
use time::util::local_offset::Soundness;
use tokio::sync::watch;

#[derive(Parser)]
pub struct Arguments {
	#[clap(env = "FIZZLE_CONFIG_PATH")]
	config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	// SAFETY: We do not modify our own environment so this is OK.
	unsafe {
		time::util::local_offset::set_soundness(Soundness::Unsound);
	}

	let arguments = Arguments::parse();
	let (shutdown_tx, shutdown_rx) = watch::channel(false);

	// Read the configuration file
	let config = load_config(arguments.config)?;

	// Setup the InfluxDB client.
	let influxdb_client =
		InfluxDbClient::new(config.influxdb.host.clone(), &config.influxdb.token)?;
	let query_client = influxdb_client.query_client().org(&config.influxdb.org);
	//
	let (write_client, influxdb_task) = if !config.influxdb.read_only {
		influxdb_client
			.write_to_bucket(&config.influxdb.bucket)
			.org(&config.influxdb.org)
			.precision(Precision::Milliseconds)
			.build()
			.buffered(shutdown_rx.clone())
	} else {
		stdout_buffered_client()
	};

	write_client
		.write_with(|builder| {
			builder
				.measurement("fizzle")
				.tag("reason", "started")
				.field("pid", std::process::id() as u64)
				.close_line()
		})
		.await?;

	// Spawn a task to handle incoming MQTT messages
	//
	let options = Options {
		host: config.mqtt.host.clone(),
		port: config
			.mqtt
			.port
			.unwrap_or_else(|| if config.mqtt.tls { 8883 } else { 1883 }),
		tls: config.mqtt.tls,
		..Default::default()
	};
	let (mqtt_client, handle) = tcp_client(options);

	// Spawn the smart-meter task.
	//
	let smart_meter_task = tokio::spawn(tasks::smart_meter::smart_meter_task(
		mqtt_client.clone(),
		write_client.clone(),
		FilterBuf::new("meter-reader/impulse/raw")?,
	));

	// Spawn a task to drive the character display device
	//
	let display_task = tasks::display::create_task(
		mqtt_client.clone(),
		query_client,
		Arc::clone(&config),
		shutdown_rx.clone(),
	);

	// Create the smart plug swarm!
	let mut tasmota_rx = mqtt_client.subscribe("tasmota/tele/#", 64).await?;
	let mut swarm: SmartPlugSwarm<HomeTasmotaTopicScheme> =
		SmartPlugSwarm::new(write_client.clone());

	loop {
		tokio::select! {
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
	drop(write_client);

	mqtt_client.disconnect().await?;
	let _ = handle.await?;

	influxdb_task.await??;
	display_task.await??;
	smart_meter_task.await??;

	Ok(())
}

fn load_config<T: AsRef<Path>>(path: T) -> anyhow::Result<Arc<Config>> {
	let path = path.as_ref();
	let config_file = File::open(path)?;
	let config = match path.extension().and_then(|s| s.to_str()) {
		Some("yaml") | Some("yml") => serde_yaml::from_reader(config_file)?,
		Some("json") => serde_json::from_reader(config_file)?,
		None | Some(_) => panic!("unknown config file extension"),
	};
	let config = Arc::new(config);
	Ok(config)
}
