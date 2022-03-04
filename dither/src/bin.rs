#![feature(try_blocks)]


use std::net::Ipv4Addr;
use std::net::SocketAddr;

use anyhow::anyhow;
use async_std::task;
use futures::SinkExt;
use futures::StreamExt;
use futures::channel::mpsc;
use libdither::{DitherCommand, DitherCore, Address};
use node::NodeID;

use rustyline::{error::ReadlineError, Editor};

#[async_std::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();
	
	let listen_port: u16 = match std::env::args().nth(1).map(|s|s.parse()) {
		Some(Ok(port)) => port,
		None => return Ok(log::error!("Requires a port number as a command line argument")),
		Some(Err(err)) => return Ok(log::error!("Failed to parse port number: {err}"))
	};
	let listen_addr = SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), listen_port);
	let (core, mut event_receiver) = DitherCore::init(listen_addr)?;
	let (mut command_sender, command_receiver) = mpsc::channel(20);
	
	let _core_join = task::spawn(core.run(command_receiver));

	let mut rl = Editor::<()>::new();
	println!("Welcome to Dither (🖧), type help for command list");
    if rl.load_history("target/.dither-history").is_err() {
       // println!("No previous history.");
    }

	let _event_join = task::spawn(async move {
		while let Some(event) = event_receiver.next().await {
			println!("{:?}", event);
		}
	});

	loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());

				let mut split = line.split(" ");
				let line = if let Some(split) = split.next() { split } else { continue };
				let ret: anyhow::Result<()> = try {
					match line {
						"connect" => {
							let node_id = split.next().map(|s|s.parse::<NodeID>()).ok_or(anyhow!("Failed to parse NodeID"))??;
							let addr = split.next().map(|s|s.parse::<Address>()).ok_or(anyhow!("Failed to parse Multiaddr"))??;
							command_sender.send(DitherCommand::Bootstrap(node_id, addr)).await?;
						}
						"info" => {
							command_sender.send(DitherCommand::GetNodeInfo).await?;
						}
						"action" => {
							
						}
						"help" => {
							println!(r"
connect <NodeID> <Address> - connect to remote device
info - get info about this node
action - wip, send node action
							")
						}
						_ => { println!("Unknown command, type help for a list of commands"); }
					}
				};
				if let Err(err) = ret { println!("Error: {}", err); }
				
            },
            Err(ReadlineError::Interrupted) => {
                // println!("CTRL-C, do CTRL-D to exit");
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
	rl.save_history("target/.dither-history").unwrap();

	// println!("event receiver closed with: {:?}", event_join.await);

	Ok(())
}