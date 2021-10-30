#![feature(try_blocks)]

#[macro_use]
extern crate serde;
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derivative;

/* #[macro_use]
extern crate bitflags; */
//#![allow(dead_code)]

use std::{collections::HashMap, fmt::{Debug}, fs::File, io::{BufReader, Write}};
use std::ops::Range;

use anyhow::Context;
use nalgebra::Vector2;

use netsim_embed::{MachineId, Netsim};
use serde::Deserialize;

use device::{DeviceCommand, DeviceEvent};
use node::{RouteCoord, net};
use tokio::{sync::mpsc, task::JoinHandle};

/// All Dither Nodes and Routing Nodes will be organized on a field
/// Internet Simulation Field Dimensions (Measured in Nanolightseconds): 64ms x 26ms
pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320000..320000, -130000..130000);
pub const DEFAULT_CACHE_FILE: &str = "./net.cache";

pub type FieldPosition = Vector2<i32>;
pub type Latency = u64;

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
#[derive(Derivative)]
#[derivative(Debug)]
struct InternetNode {
	/// Position of this Node in the Internet Simulation Field
	position: FieldPosition,
	/// Internal latency of this node's internal network and packet processing (Measure in nanoseconds)
	internal_latency: Latency,
}

#[derive(Debug)]
pub enum InternetAction {
	AddNode,
	AddNetwork,
	SetPosition(usize),
}
#[derive(Debug, Clone)]
pub enum InternetEvent {
	AddNodeResp(usize),
	AddNetworkResp(usize),

	Error(String),
}

#[derive(Error, Debug)]
pub enum InternetError {
	#[error("There is no node for this NetAddr: {net_addr:?}")]
	NoNodeError { net_addr: net::Address },
	#[error("Event Receiver Closed")]
	EventReceiverClosed,

	#[error(transparent)]
	Other(#[from] anyhow::Error),

}

#[derive(Derivative, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct Internet {
	#[derivative(Debug="ignore")]
	#[serde(skip)]
	pub netsim: Netsim<DeviceCommand, DeviceEvent>,
	pub route_coord_dht: HashMap<node::NodeID, RouteCoord>,
}

impl Internet {
	pub fn new() -> Internet {
		Internet {
			netsim: Netsim::new(),
			route_coord_dht: HashMap::new(),
		}
	}
	pub fn run(mut self) -> (mpsc::Sender<InternetAction>, mpsc::Receiver<InternetEvent>, JoinHandle<()>) {
		let (action_sender, mut action_receiver) = mpsc::channel(20);
		let (event_sender, event_receiver) = mpsc::channel(20);
		let join = tokio::spawn(async move {
			while let Some(action) = action_receiver.recv().await {
				let res: Result<(), InternetError> = try {
					match action {
						InternetAction::AddNode => {
							let machine_id = self.add_node().await;
							event_sender.send(InternetEvent::AddNodeResp(machine_id.0)).await.map_err(|_|InternetError::EventReceiverClosed)?;
						},
						_ => log::error!("Unimplemented InternetAction: {:?}", action),
					}
				};
				if let Err(err) = res {
					if let Err(err) = event_sender.send(InternetEvent::Error(format!("{:?}", err))).await {
						log::error!("Internet Event Receiver Closed: {:?}", err);
						break;
					}
				}
			}
		});
		(action_sender, event_receiver, join)
	}
	pub fn lease_id(&self) -> usize {
		self.netsim.machines().len()
	}
	pub async fn add_node(&mut self) -> MachineId {
		self.netsim.spawn_machine(async_process::Command::new("./target/debug/device"), None).await
	}
	pub fn save(&self, filepath: &str) -> Result<(), InternetError> {
		let mut file = File::create(filepath).context("failed to create file (check perms) at {}")?;
		let data = bincode::serialize(&self).context("failed to serialize network")?;
		file.write_all(&data).context("failed to write to file")?;
		Ok(())
	}
	pub fn load(&mut self, filepath: &str) -> Result<(), InternetError> {
		let file = File::open(filepath).context("failed to open file (check perms)")?;
		*self = bincode::deserialize_from(BufReader::new(file)).context("failed to deserialize network")?;
		Ok(())
	}
}