//! Standalone executable for dither-core to be run by simulation. Commands sent via stdin/stdout

#![feature(try_blocks)]

use std::{str::FromStr, thread};

use libdither::{DitherCore, commands::DitherCommand};
use tokio::sync::mpsc;

mod types;
pub use types::{DeviceCommand, DeviceEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let (event_sender, mut event_receiver) = mpsc::channel(20);
	macro_rules! resp_error{
		($($arg:tt)*) => {{
			let _ = event_sender.send(DeviceEvent::Error(format!($($arg)*))).await;
		}}
	}
	macro_rules! resp_debug{
		($($arg:tt)*) => {{
			let _ = event_sender.send(DeviceEvent::Debug(format!($($arg)*))).await;
		}}
	}

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
	let dither_core = DitherCore::init(listen_addr)?;
	let dither_core_thread = tokio::spawn(async move {
		dither_core.run().await
	});

	// Main Thread
	let main_thread = tokio::spawn(async move {
		tokio::select! {
			command = command_receiver.recv() => {
				let result: anyhow::Result<()> = try {
					if let Some(command) = command {
						resp_debug!("Received valid DeviceCommand: {:?}", command);
						match command {
							DeviceCommand::Connect(ip, port) => {
								resp_debug!("Received command to connect to: {:?}:{:?}", ip, port);
								//event_sender.send(DeviceEvent::Error(format!("Received command to connect to {:?}:{:?}", ip, port))).await;
							},
							DeviceCommand::DitherCommand(dither_command) => {
								match dither_command {
									DitherCommand::Bootstrap(id, addr) => {
										resp_debug!("Received command to bootstrap to Node({:?}) at {:?}", id, String::from_utf8_lossy(&addr.0));
									}
									_ => { resp_error!("DitherCommand: Unimplemented command: {:?}", dither_command) }
								}
							},
							_ => resp_error!("Unimplemented command: {:?}", command)
						}
					}
				};
				if let Err(err) = result {
					resp_error!("Error: {:?}", err);
				}
			}
		}
	});

	let rets = tokio::join!(parse_input_commands.join().expect("input command thread failed"), parse_events, main_thread, dither_core_thread);
	let _ = rets.3??;

	//tokio::join!(parse_input_commands).await;

	Ok(())
}

