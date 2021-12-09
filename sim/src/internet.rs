#![allow(unused)]

use std::{collections::HashMap, fmt::{Debug}, fs::File, io::{BufReader, Write}, net::Ipv4Addr, time::Duration};
use std::ops::Range;

use anyhow::Context;
use async_std::task;
use nalgebra::Vector2;
use petgraph::{graph::NodeIndex, Graph};
use netsim_embed::{Ipv4Range, Machine, MachineId, Netsim, Network};
use serde::Deserialize;

use device::{Address, DeviceCommand, DeviceEvent};
use node::{NodeID, RouteCoord, net};
use futures::{SinkExt, Stream, StreamExt, channel::mpsc, stream};

mod netsim_ext;
mod internet_node;
use netsim_ext::*;

use self::internet_node::{Latency, MachineInfo, NetworkInfo, NodeInfo};
pub use self::internet_node::{FieldPosition, InternetMachine, InternetNode, NodeType};

/// All Dither Nodes and Routing Nodes will be organized on a field
/// Internet Simulation Field Dimensions (Measured in Nanolightseconds): 64ms x 26ms
pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320000..320000, -130000..130000);
pub const DEFAULT_CACHE_FILE: &str = "./net.cache";
pub const DEFAULT_INTERNAL_LATENCY: u64 = 20;
pub const MAX_NETWORKS: u16 = u16::MAX;

#[derive(Debug, Serialize, Deserialize)]
pub enum InternetAction {
	AddMachine(FieldPosition),
	AddNetwork(FieldPosition),
	GetNodeInfo(usize), // Get info about node
	GetMachineInfo(usize), // Get info about machine
	GetNetworkInfo(usize), // Get info about network
	SendMachineAction(usize),

	SetPosition(usize),
	DeviceCommand(usize, DeviceCommand),

	// From Devices
	DeviceEvent(usize, DeviceEvent)
}
#[derive(Debug, Clone)]
pub enum InternetEvent {
	NewMachine(usize),
	NewNetwork(usize),
	NodeInfo(NodeInfo),
	MachineInfo(MachineInfo),
	NetworkInfo(NetworkInfo),

	Error(String),
}

#[derive(Error, Debug)]
pub enum InternetError {
	#[error("Event Receiver Closed")]
	EventReceiverClosed,
	#[error("Spawned Too many networks, not enough addresses (see MAX_NETWORKS)")]
	TooManyNetworks,

	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/* #[derive(Derivative, Serialize, Deserialize)]
#[derivative(Debug)] */
pub struct Internet {
	network: Graph<InternetNode, Wire>,

	action_receiver: Option<mpsc::Receiver<InternetAction>>,
	action_sender: mpsc::Sender<InternetAction>,
	pub event_sender: mpsc::Sender<InternetEvent>,

	ip_range_iter: Box<dyn Iterator<Item = Ipv4Range> + Send>,
}

impl Internet {
	pub fn new() -> (Internet, mpsc::Receiver<InternetEvent>, mpsc::Sender<InternetAction>) {
		let (event_sender, event_receiver) = mpsc::channel(20);
		let (action_sender, action_receiver) = mpsc::channel(20);
		
		let action_sender_ret = action_sender.clone();
		(Internet {
			network: Graph::default(),
			action_receiver: Some(action_receiver),
			action_sender,
			event_sender,
			ip_range_iter: Box::new(Ipv4Range::global_split(MAX_NETWORKS as u32)),
		}, event_receiver, action_sender_ret)
	}

	/// WARNING: This function should be called when in ushare() context
	pub async fn run(mut self) {
		let mut action_receiver = self.action_receiver.take().expect("there should be an action receiver here");
		while let Some(action) = action_receiver.next().await {
			let res: Result<(), InternetError> = try {
				println!("{:?}", action);
				match action {
					InternetAction::AddNetwork(position) => {
						let idx: NodeIndex = self.spawn_network(position).await?;
						self.send_event(InternetEvent::NewMachine(idx.index()));
						log::debug!("Added Network Node: {:?}", idx);
					}
					InternetAction::AddMachine(position) => {
						let idx = self.spawn_machine(position).await;
						log::debug!("Added Machine Node: {:?}", idx);
					}
					_ => println!("Unimplemented Action")
				}
			};
			if let Err(err) = res {
				if let Err(_) = self.send_event(InternetEvent::Error(format!("{:?}", err))).await {
					log::error!("InternetEvent receiver closed unexpectedly");
					break;
				}
			}
		}
	}
	pub async fn send_event(&mut self, event: InternetEvent) -> Result<(), InternetError> {
		self.event_sender.send(event).await.map_err(|err|InternetError::EventReceiverClosed)
	}
	pub async fn spawn_machine(&mut self, position: FieldPosition) -> NodeIndex {
		let machine_id = self.network.node_count();

		let (plug_to_wire, machine_plug) = netsim_embed::wire();

		let event_sender = self.event_sender.clone();
		
		let (machine, mut device_event_receiver) = Machine::new(MachineId(machine_id), machine_plug, async_process::Command::new("./target/debug/device")).await;
		let mut action_sender = self.action_sender.clone();
		let event_join_handle = task::spawn(async move { 
			while let Some(device_event) = device_event_receiver.next().await {
				action_sender.send(InternetAction::DeviceEvent(machine_id, device_event));
			}
		});

		let (outgoing_plug, plug_from_wire) = netsim_embed::wire();
		let wire = Wire { delay: Duration::from_millis(DEFAULT_INTERNAL_LATENCY) };
		let internet_machine = InternetMachine {
			machine,
			event_join_handle,
			unconnected_plug: Some(outgoing_plug),
			internal_wire: wire.connect(plug_to_wire, plug_from_wire),
		};

		let node = InternetNode::from_machine(internet_machine, position).await;
		self.network.add_node(node)
	}
	pub async fn spawn_network(&mut self, position: FieldPosition) -> Result<NodeIndex, InternetError> {
		let id = netsim_embed::NetworkId(self.network.node_count());
		let range = self.ip_range_iter.next().ok_or(InternetError::TooManyNetworks)?;

		let network = Network::new(id, range);
		let node = InternetNode::from_network(network, position).await;
		Ok(self.network.add_node(node))
	}
	/// Connect Machine to Network, or Network to Network.
	/// Returns error if: Already connected or both nodes are Machines
	pub async fn connect(&mut self, node_a: NodeIndex, node_b: NodeIndex) -> Result<(), InternetError> {
		Ok(())
	}
	pub async fn disconnect(&mut self, node_a: NodeIndex, node_b: NodeIndex) -> Result<(), InternetError> {
		Ok(())
	}
	/* pub fn save(&self, filepath: &str) -> Result<(), InternetError> {
		let mut file = File::create(filepath).context("failed to create file (check perms) at {}")?;
		let data = bincode::serialize(&self).context("failed to serialize network")?;
		file.write_all(&data).context("failed to write to file")?;
		Ok(())
	}
	pub fn load(&mut self, filepath: &str) -> Result<(), InternetError> {
		let file = File::open(filepath).context("failed to open file (check perms)")?;
		*self = bincode::deserialize_from(BufReader::new(file)).context("failed to deserialize network")?;
		Ok(())
	} */
}