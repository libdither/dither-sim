//! Standalone executable for dither-core to be run by simulation. Commands sent via stdin/stdout

#![feature(try_blocks)]

use std::{net::SocketAddr, str::FromStr};
use async_std::{task};
use futures::{FutureExt, StreamExt, SinkExt, channel::mpsc};

use libdither::{DitherCore, commands::DitherCommand};

mod types;
pub use types::{DeviceCommand, DeviceEvent};

use anyhow::{Context, anyhow};

#[async_std::main]
async fn main() -> anyhow::Result<()> {
	let (mut event_sender, mut event_receiver) = mpsc::channel(20);
	/* macro_rules! resp_debug{
		($($arg:tt)*) => {{
			let _ = event_sender.send(DeviceEvent::Debug(format!($($arg)*))).await;
		}}
	} */

	// Stdout parsing thread
	let parse_events = task::spawn(async move {
		while let Some(event) = event_receiver.next().await {
			println!("<{}", event); // Print to stdout, requires '<' to be marked as event
		}
	});

	let (mut command_sender, mut command_receiver) = mpsc::channel(20);
	// Stdin parsing thread
	let parse_input_commands = task::spawn(async move {
		let stdin = async_std::io::stdin();
		let mut input = String::new();
		while let Ok(_) = stdin.read_line(&mut input).await {
			if let Ok(command) = DeviceCommand::from_str(&input) {
				command_sender.send(command).await.expect("Command Sender should be open");
			} else {
				println!("Invalid DeviceCommand (must be RON-formatted string): {:?}", input);
			}
			input.clear();
		}
		()
	});
	
	let listen_addr = SocketAddr::from_str("/ip4/0.0.0.0/tcp/3000")?;
	let (dither_core, mut dither_event_receiver) = DitherCore::init(listen_addr)?;
	let (mut dither_command_sender, dither_command_receiver) = mpsc::channel(20);
	let dither_core_thread = task::spawn(async move {
		dither_core.run(dither_command_receiver).await
	});

	// Main Thread for Device
	let main_thread = task::spawn(async move {
		loop {
			futures::select! {
				dither_event = dither_event_receiver.next().fuse() => {
					let result: anyhow::Result<()> = try {
						match dither_event.ok_or(anyhow!("failed to receive DitherEvent"))? {
							event => event_sender.try_send(DeviceEvent::DitherEvent(event)).context("failed to send device event")?
						}
					};
					if let Err(err) = result {
						println!("Failed to send Device Event: {:?}", err);
						event_sender.try_send(DeviceEvent::Error(format!("{:?}", err))).unwrap();
					}
				}
				command = command_receiver.next().fuse() => {
					let result: anyhow::Result<()> = try {
						match command.ok_or(anyhow!("Failed to receive DeviceCommand"))? {
							DeviceCommand::DitherCommand(dither_command) => {
								dither_command_sender.try_send(dither_command)?;
							},
							// command => Err(anyhow!("Unimplemented DeviceCommand: {:?}", command))?,
						}
					};
					if let Err(err) = result { event_sender.try_send(DeviceEvent::Error(format!("{:?}", err))).unwrap(); }
				}
			}
		}
		
	});

	parse_input_commands.await;
	parse_events.await;
	main_thread.await;
	dither_core_thread.await?;

	Ok(())
}

