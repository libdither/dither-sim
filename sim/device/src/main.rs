//! Standalone executable for dither-core to be run by simulation. Commands sent via stdin/stdout

#![feature(try_blocks)]

use std::{str::FromStr, thread};

use libdither::{DitherCore, commands::DitherCommand};
use tokio::sync::mpsc;

mod types;
pub use types::{DeviceCommand, DeviceEvent};

use anyhow::{Context, anyhow};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let (event_sender, mut event_receiver) = mpsc::channel(20);
	/* macro_rules! resp_debug{
		($($arg:tt)*) => {{
			let _ = event_sender.send(DeviceEvent::Debug(format!($($arg)*))).await;
		}}
	} */

	// Stdout parsing thread
	let parse_events = tokio::spawn(async move {
		while let Some(event) = event_receiver.recv().await {
			println!("{}", event); // Print to stdout
		}
	});

	let (command_sender, mut command_receiver) = mpsc::channel(20);
	// Stdin parsing thread
	let parse_input_commands = thread::spawn(|| async move {
		let stdin = std::io::stdin();
		let mut input = String::new();
		while let Ok(_) = stdin.read_line(&mut input) {
			if let Ok(command) = DeviceCommand::from_str(&input) {
				command_sender.send(command).await.expect("Command Sender should be open");
			} else {
				println!("Invalid DeviceCommand (must be RON-formatted string): {:?}", input);
			}
			input.clear();
		}
		()
	});
	
	let listen_addr = libdither::Multiaddr::from_str("/ip4/0.0.0.0/tcp/3000")?;
	let (dither_core, mut dither_event_receiver) = DitherCore::init(listen_addr)?;
	let (dither_command_sender, dither_command_receiver) = mpsc::channel(20);
	let dither_core_thread = tokio::spawn(async move {
		dither_core.run(dither_command_receiver).await
	});

	// Main Thread for Device
	let main_thread = tokio::spawn(async move {
		loop {
			tokio::select! {
				dither_event = dither_event_receiver.recv() => {
					println!("Received event: {dither_event:?}");
					let result: anyhow::Result<()> = try {
						match dither_event.ok_or(anyhow!("failed to receive DitherEvent"))? {
							event => event_sender.try_send(DeviceEvent::DitherEvent(event)).context("failed to send device event")?
						}
					};
					if let Err(err) = result { event_sender.try_send(DeviceEvent::Error(format!("{:?}", err))).unwrap(); }
				}
				command = command_receiver.recv() => {
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

	let rets = tokio::join!(parse_input_commands.join().expect("input command thread failed"), parse_events, main_thread, dither_core_thread);
	let _ = rets.3??;

	//tokio::join!(parse_input_commands).await;

	Ok(())
}

