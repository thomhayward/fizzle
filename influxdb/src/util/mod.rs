use std::{io::Read, time::Duration};

use crate::{buffered, Status};
use bytes::{Buf, Bytes, BytesMut};
use tokio::{
	sync::{mpsc, watch},
	task::JoinHandle,
	time::interval,
};

pub fn stdout_buffered_client() -> (buffered::Client, JoinHandle<anyhow::Result<()>>) {
	let (tx, mut rx) = mpsc::channel::<(Bytes, watch::Sender<Status>)>(64);

	let handle = tokio::spawn(async move {
		let mut shutdown = false;
		let mut buffer = BytesMut::with_capacity(32768);
		let mut flush_interval = interval(Duration::from_secs(30));

		while !shutdown {
			let flush = tokio::select! {
			  message = rx.recv() => {
				match message {
				  Some((buf, status)) => {
					buffer.extend_from_slice(&buf);
					status.send_replace(Status::Buffered);

					let total_lines = buffer.iter().filter(|&x| x == &b'\n').count();
					total_lines >= 5000 || buffer.len() >= 30_000
				  }
				  None => {
					shutdown = true;
					true
				  },
				}
			  }
			  _ = flush_interval.tick() => {
				!buffer.is_empty()
			  }
			};

			// Print the buffered line-protocol to stdout.
			if flush {
				let mut output = String::with_capacity(buffer.len());
				let mut reader = buffer.reader();
				reader.read_to_string(&mut output)?;
				print!("{}", output);

				buffer = BytesMut::with_capacity(32768);
			}
		}
		Ok(())
	});

	(buffered::Client::new(tx), handle)
}
