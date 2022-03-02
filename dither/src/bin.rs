#![feature(try_blocks)]


use anyhow::Context;
use anyhow::anyhow;
use futures::join;
use libdither::{DitherCommand, DitherCore, Address};
use node::NodeID;

use rustyline::{error::ReadlineError, Editor};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();
	
	let listen_addr: (Ipv4Addr, u16) = (Ipv4Addr::new(0, 0, 0, 0), &std::env::args().nth(1).unwrap().parse().unwrap());
	let (core, mut event_receiver) = DitherCore::init(listen_addr)?;
	let (command_sender, command_receiver) = mpsc::channel(20);
	
	let _core_join = tokio::spawn(core.run(command_receiver));

	let mut rl = Editor::<()>::new();
	println!("Welcome to disp (λ)");
    if rl.load_history(".dither-history").is_err() {
       // println!("No previous history.");
    }

	let _event_join = tokio::spawn(async move {
		while let Some(event) = event_receiver.recv().await {
			println!("{:?}", event);
		}
	});

	loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());

				let mut split = line.split(" ");
				let line = if let Some(split) = split.next() { split } else { continue };
				let ret: anyhow::Result<()> = try {
					match line {
						"connect" => {
							let node_id = split.next().map(|s|s.parse::<PeerId>()).ok_or(anyhow!("Failed to parse PeerID"))??.to_bytes().into();
							let addr = Address(split.next().map(|s|s.parse::<Multiaddr>()).ok_or(anyhow!("Failed to parse Multiaddr"))??.to_vec());
							command_sender.send(DitherCommand::Bootstrap(node_id, addr)).await?;
						}
						"info" => {
							command_sender.send(DitherCommand::GetNodeInfo).await?;
						}
						"action" => {
							
						}
						_ => { println!("Unknown command"); }
					}
				};
				if let Err(err) = ret { println!("Error: {}", err); }
				
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
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
	rl.save_history(".dither-history").unwrap();

	// println!("event receiver closed with: {:?}", event_join.await);

	Ok(())
}