#![allow(dead_code)]

/// Internet Simulation Module
/// Contains all necessary componenets to create a virtual network on a given computer and spawn devices running the Dither protocol

use std::{fmt::{Debug}, time::Duration};
use std::ops::Range;

use async_std::task;
use petgraph::{graph::NodeIndex, Graph};
use netsim_embed::{Ipv4Range, Machine, MachineId, Network};
use serde::Deserialize;

use device::{DeviceCommand, DeviceEvent};
use node::{NodeID, RouteCoord, net};
use futures::{SinkExt, StreamExt, channel::mpsc};

mod netsim_ext;
mod internet_node;
use netsim_ext::*;

use self::internet_node::InternetMachineConnection;
pub use self::internet_node::{FieldPosition, InternetMachine, InternetNode, NodeType, NodeInfo, MachineInfo, NetworkInfo, Latency};

/// All Dither Nodes and Routing Nodes will be organized on a field
/// Internet Simulation Field Dimensions (Measured in Nanolightseconds): 64ms x 26ms
pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320000..320000, -130000..130000);

/// Cache file to save network configuration
pub const DEFAULT_CACHE_FILE: &str = "./net.cache";

/// Default internal latency (measured in millilightseconds)
pub const DEFAULT_INTERNAL_LATENCY: u64 = 20;
/// Max number of networks allowed (represents how many slices the global IP space is split into).
pub const MAX_NETWORKS: u16 = u16::MAX;

/// Internet Simulation Actions, use this structure to control the simulation thread
#[derive(Debug, Serialize, Deserialize)]
pub enum InternetAction {
	/// Add Machine at a specific position in simulation space
	AddMachine(FieldPosition),
	/// Add Network at a specific position in simulation space
	AddNetwork(FieldPosition),
	/// Get info about a given node, machine or network (takes node ID) -> NodeInfo
	GetNodeInfo(usize), // Get info about node
	/// Get info about a given Machine running Dither -> MachineInfo
	GetMachineInfo(usize), // Get info about machine
	/// Get info about a given Network thread -> NetworkInfo
	GetNetworkInfo(usize), // Get info about network
	//Send Dither-specific action to a machine?
	///SendMachineAction(usize),

	/// Change position of a given node in the network
	SetPosition(usize, FieldPosition),

	/// Send Device command (Dither-specific or otherwise) -> NodeInfo & MachineInfo
	DeviceCommand(usize, DeviceCommand),

	// From Devices
	DeviceEvent(usize, DeviceEvent)
}

/// Internet Simulation Events, use this structure to listen to events from the simulation thread
#[derive(Debug, Clone)]
pub enum InternetEvent {
	/// New machine was created
	NewMachine(usize),
	/// Net network was created
	NewNetwork(usize),
	/// General Node info 
	NodeInfo(usize, Option<NodeInfo>),
	/// General machine info
	MachineInfo(usize, Option<MachineInfo>),
	/// General network info
	NetworkInfo(usize, Option<NetworkInfo>),

	/// Error
	Error(String),
}

/// Internet Error object
#[derive(Error, Debug)]
pub enum InternetError {
	#[error("Event Receiver Closed")]
	EventReceiverClosed,
	#[error("Action Sender Closed")]
	ActionSenderClosed,
	#[error("Spawned Too many networks, not enough addresses (see MAX_NETWORKS)")]
	TooManyNetworks,

	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/// Internet object, contains handles to the network and machine threads
/* #[derive(Derivative, Serialize, Deserialize)]
#[derivative(Debug)] */
pub struct Internet {
	network: Graph<InternetNode, Wire>,

	action_receiver: Option<mpsc::Receiver<InternetAction>>,
	action_sender: mpsc::Sender<InternetAction>,
	pub event_sender: mpsc::Sender<InternetEvent>,

	device_exec: String,

	ip_range_iter: Box<dyn Iterator<Item = Ipv4Range> + Send>,
}

impl Internet {
	/// Create new internet instance with action senders and event receivers
	pub fn new() -> (Internet, mpsc::Receiver<InternetEvent>, mpsc::Sender<InternetAction>) {
		let (event_sender, event_receiver) = mpsc::channel(20);
		let (action_sender, action_receiver) = mpsc::channel(20);
		
		let action_sender_ret = action_sender.clone();
		(Internet {
			network: Graph::default(),
			action_receiver: Some(action_receiver),
			action_sender,
			event_sender,
			device_exec: "./target/debug/device".into(),
			ip_range_iter: Box::new(Ipv4Range::global_split(MAX_NETWORKS as u32)),
		}, event_receiver, action_sender_ret)
	}
	/// Run network function
	/// IMPORTANT: This function must be called from an unshare() context (i.e. a kernel virtual network)
	pub async fn run(mut self) {
		std::fs::metadata(&self.device_exec).expect("no device file!");

		let mut action_receiver = self.action_receiver.take().expect("there should be an action receiver here");
		while let Some(action) = action_receiver.next().await {
			let res: Result<(), InternetError> = try {
				println!("Received Internet Action: {:?}", action);
				match action {
					InternetAction::AddNetwork(position) => {
						let idx: NodeIndex = self.spawn_network(position).await?;
						self.send_event(InternetEvent::NewNetwork(idx.index())).await?;
						log::debug!("Added Network Node: {:?}", idx);
					}
					InternetAction::AddMachine(position) => {
						let idx = self.spawn_machine(position).await;
						self.send_event(InternetEvent::NewMachine(idx.index())).await?;
						self.action(InternetAction::GetNodeInfo(idx.index())).await?;
						log::debug!("Added Machine Node: {:?}", idx);
					}
					InternetAction::GetNodeInfo(index) => {
						let node = self.network.node_weight(NodeIndex::<u32>::new(index));
						let node_info = node.map(|n|n.gen_node_info());
						self.send_event(InternetEvent::NodeInfo(index, node_info)).await?;
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
	/// Send event function (used internally by run())
	async fn send_event(&mut self, event: InternetEvent) -> Result<(), InternetError> {
		self.event_sender.send(event).await.map_err(|err|InternetError::EventReceiverClosed)
	}
	async fn action(&mut self, action: InternetAction) -> Result<(), InternetError> {
		self.action_sender.send(action).await.map_err(|err|InternetError::ActionSenderClosed)
	}
	/// Spawn machine at position
	async fn spawn_machine(&mut self, position: FieldPosition) -> NodeIndex {
		let machine_id = self.network.node_count();

		let (plug_to_wire, machine_plug) = netsim_embed::wire();

		let event_sender = self.event_sender.clone();
		
		let (machine, mut device_event_receiver) = Machine::new(MachineId(machine_id), machine_plug, async_process::Command::new(&self.device_exec)).await;
		let mut action_sender = self.action_sender.clone();
		let event_join_handle = task::spawn(async move { 
			while let Some(device_event) = device_event_receiver.next().await {
				action_sender.send(InternetAction::DeviceEvent(machine_id, device_event)).await;
			}
		});

		let (outgoing_plug, plug_from_wire) = netsim_embed::wire();
		let wire = Wire { delay: Duration::from_millis(DEFAULT_INTERNAL_LATENCY) };
		let internet_machine = InternetMachine {
			machine,
			event_join_handle,
			connection_status: InternetMachineConnection::Unconnected(outgoing_plug),
			internal_wire: wire.connect(plug_to_wire, plug_from_wire),
			internal_latency: DEFAULT_INTERNAL_LATENCY,
		};

		let node = InternetNode::from_machine(internet_machine, position);
		self.network.add_node(node)
	}
	/// Spawn network at position
	async fn spawn_network(&mut self, position: FieldPosition) -> Result<NodeIndex, InternetError> {
		let id = netsim_embed::NetworkId(self.network.node_count());
		let range = self.ip_range_iter.next().ok_or(InternetError::TooManyNetworks)?;

		let network = Network::new(id, range);
		let node = InternetNode::from_network(network, position);
		Ok(self.network.add_node(node))
	}
	/* /// Connect Machine to Network, or Network to Network.
	/// Returns error if: Already connected or both nodes are Machines
	async fn connect(&mut self, node_a: NodeIndex, node_b: NodeIndex) -> Result<(), InternetError> {
		Ok(())
	}
	async fn disconnect(&mut self, node_a: NodeIndex, node_b: NodeIndex) -> Result<(), InternetError> {
		Ok(())
	} */
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