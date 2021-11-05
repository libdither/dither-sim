#![allow(unused)]

use std::{collections::HashMap, fmt::{Debug}, fs::File, io::{BufReader, Write}};
use std::ops::Range;

use anyhow::Context;
use nalgebra::Vector2;

use netsim_embed::{MachineId, Netsim};
use serde::Deserialize;

use device::{DeviceCommand, DeviceEvent};
use node::{RouteCoord, net};
use futures::{SinkExt, StreamExt, channel::mpsc};

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

#[derive(Debug, Serialize, Deserialize)]
pub enum InternetAction {
	AddNode,
	AddNetwork,
	SetPosition(usize),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
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
	nodes: Vec<usize>,
}

impl Internet {
	pub fn new() -> Internet {
		Internet {
			netsim: Netsim::new(),
			route_coord_dht: HashMap::new(),
			nodes: Default::default(),
		}
	}

	/// WARNING: This function should be called using netsim_embed::run from a single-threaded context
	pub async fn run(mut self, mut event_sender: mpsc::Sender<InternetEvent>, mut action_receiver: mpsc::Receiver<InternetAction>) {
		while let Some(action) = action_receiver.next().await {
			let res: Result<(), InternetError> = try {
				println!("{:?}", action);
				match action {
					InternetAction::AddNetwork => { log::debug!("AddNetwork") }
					InternetAction::AddNode => {
						let id = self.add_node().await;
						log::debug!("Added Node: {}", id);
						self.nodes.push(id.0);
					}
					_ => println!("Unimplemented Action")
				}
			};
			if let Err(err) = res {
				if let Err(err) = event_sender.send(InternetEvent::Error(format!("{:?}", err))).await {
					log::error!("Internet Event Receiver Closed: {:?}", err);
					break;
				}
			}
		}
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