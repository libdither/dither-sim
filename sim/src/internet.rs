#![allow(dead_code)]

/// Internet Simulation Module
/// Contains all necessary componenets to create a virtual network on a given computer and spawn devices running the Dither protocol

use std::fmt::{Debug};
use std::ops::Range;
use std::time::Duration;

use petgraph::{graph::NodeIndex, Graph};
use netsim_embed::Ipv4Range;
use serde::Deserialize;

use device::{DeviceCommand, DeviceEvent, DitherEvent};
use futures::{SinkExt, StreamExt, channel::mpsc};

mod netsim_ext;
mod internet_node;
use netsim_ext::*;

pub use self::internet_node::{FieldPosition, InternetNetwork, InternetMachine, InternetNode, NodeType, NodeInfo, MachineInfo, NetworkInfo, Latency, NodeVariant};

/// All Dither Nodes and Routing Nodes will be organized on a field
/// Internet Simulation Field Dimensions (Measured in Microlightseconds): 64ms x 26ms
pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320000..320000, -130000..130000);

/// Cache file to save network configuration
pub const DEFAULT_CACHE_FILE: &str = "./net.cache";

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
	/// Connect two nodes
	ConnectNodes(usize, usize),

	/// Send Device command (Dither-specific or otherwise) -> NodeInfo & MachineInfo
	DeviceCommand(usize, DeviceCommand),

	// From Devices
	HandleDeviceEvent(usize, DeviceEvent)
}

/// Internet Simulation Events, use this structure to listen to events from the simulation thread
#[derive(Debug, Clone)]
pub enum InternetEvent {
	/// New machine was created
	NewMachine(usize),
	/// Net network was created
	NewNetwork(usize),
	/// General Node info 
	NodeInfo(usize, NodeInfo),
	/// General machine info
	MachineInfo(usize, MachineInfo),
	/// General network info
	NetworkInfo(usize, NetworkInfo),
	/// Connection Info
	NewConnection(usize, usize),

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
	#[error("Device Command Sender Closed")]
	DeviceCommandSenderClosed,
	#[error("Invalid Node Type for {index}, expected: {expected:?}")]
	InvalidNodeType { index: usize, expected: NodeType },
	#[error("Unknown Node index: {index}")]
	UnknownNode { index: usize },
	#[error("Can't connect machines directory to each other")]
	MachineConnectionError,

	#[error("Spawned Too many networks, not enough addresses (see MAX_NETWORKS)")]
	TooManyNetworks,

	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/// Internet object, contains handles to the network and machine threads
/* #[derive(Derivative, Serialize, Deserialize)]
#[derivative(Debug)] */
pub struct Internet {
	// DO NOT CALL .remove_node()!!! (indicies should be static)
	network: Graph<InternetNode, Wire>,

	action_receiver: Option<mpsc::Receiver<InternetAction>>,
	action_sender: mpsc::Sender<InternetAction>,
	pub event_sender: mpsc::Sender<InternetEvent>,

	device_exec: String,

	ip_range_iter: Box<dyn Iterator<Item = Ipv4Range> + Send + Sync>,
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
	fn node(&self, index: usize) -> Result<&InternetNode, InternetError> {
		self.network.node_weight(NodeIndex::new(index)).ok_or(InternetError::UnknownNode { index })
	}
	fn node_mut(&mut self, index: usize) -> Result<&mut InternetNode, InternetError> {
		self.network.node_weight_mut(NodeIndex::new(index)).ok_or(InternetError::UnknownNode { index })
	} 
	pub fn machine(&self, index: usize) -> Result<&InternetMachine, InternetError> {
		self.node(index)?.machine().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Machine })
	}
	pub fn network(&self, index: usize) -> Result<&InternetNetwork, InternetError> {
		self.node(index)?.network().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Network })
	}
	pub fn network_mut(&mut self, index: usize) -> Result<&mut InternetNetwork, InternetError> {
		self.node_mut(index)?.network_mut().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Network })
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
						self.action(InternetAction::GetNodeInfo(idx.index())).await?;
						self.action(InternetAction::GetNetworkInfo(idx.index())).await?;
						log::debug!("Added Network Node: {:?}", idx);
					}
					InternetAction::AddMachine(position) => {
						let idx = self.spawn_machine(position).await;
						self.send_event(InternetEvent::NewMachine(idx.index())).await?;
						self.action(InternetAction::GetNodeInfo(idx.index())).await?;
						self.action(InternetAction::GetMachineInfo(idx.index())).await?;
						log::debug!("Added Machine Node: {:?}", idx);
					}
					InternetAction::ConnectNodes(from, to) => {
						use NodeVariant::*;
						let node1 = self.node(from)?;
						let node2 = self.node(to)?;
						match (&node1.variant, &node2.variant) {
							(Network(net1), Network(net2)) => {
								let delay = Duration::from_micros(node1.latency_distance(node2));
								let (plug1, plug2, wire_handle) = Wire::new(delay);
								let (route1, route2) = (net1.route(), net2.route());
								let (unique_id_1, unique_id_2) = (net1.unique_id(), net2.unique_id());
								self.network_mut(from)?.connect(unique_id_2, plug1, vec![route1], wire_handle.clone());
								self.network_mut(to)?.connect(unique_id_1, plug2, vec![route2], wire_handle.clone());
								self.send_event(InternetEvent::NewConnection(from, to)).await?;
								self.action(InternetAction::GetNodeInfo(from)).await?;
								self.action(InternetAction::GetNodeInfo(to)).await?;
								self.action(InternetAction::GetNetworkInfo(from)).await?;
								self.action(InternetAction::GetNetworkInfo(to)).await?;
							}
							(Network(net), Machine(machine)) | (Machine(machine), Network(net)) => {
								
							}
							_ => Err(InternetError::MachineConnectionError)?,
						}
					}
					InternetAction::SetPosition(node, position) => {
						//self.machine(node)?.update_position(position).await?;
					}
					InternetAction::GetNodeInfo(index) => {
						self.send_event(InternetEvent::NodeInfo(index, self.node(index)?.node_info())).await?;
					}
					InternetAction::GetMachineInfo(index) => {
						// This is sent back from the Device through DeviceEvents
						self.machine(index)?.request_machine_info()?;
					}
					InternetAction::GetNetworkInfo(index) => {
						self.send_event(InternetEvent::NetworkInfo(index, self.network(index)?.network_info())).await?;

					}
					InternetAction::HandleDeviceEvent(index, DeviceEvent::DitherEvent(dither_event)) => {
						match dither_event {
							DitherEvent::NodeInfo(device::NodeInfo { route_coord, node_id, public_addr, remotes, active_remotes } ) => {
								//let machine = self.machine(index)?;
								self.send_event(InternetEvent::MachineInfo(index, MachineInfo {
									route_coord, public_addr, node_id, remotes, active_remotes
								})).await?;
							}
							//_ => log::error!("Unhandled Device Event")
						}
					}
					_ => log::error!("Unimplemented Internet Action")
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
		self.event_sender.send(event).await.map_err(|_|InternetError::EventReceiverClosed)
	}
	async fn action(&mut self, action: InternetAction) -> Result<(), InternetError> {
		self.action_sender.send(action).await.map_err(|_|InternetError::ActionSenderClosed)
	}
	/// Spawn machine at position
	async fn spawn_machine(&mut self, position: FieldPosition) -> NodeIndex {
		let machine_id = self.network.node_count();

		let machine = InternetMachine::new(machine_id, self.action_sender.clone(), &self.device_exec).await;

		let node = InternetNode::from_machine(machine, position, machine_id);
		self.network.add_node(node)
	}
	/// Spawn network at position
	async fn spawn_network(&mut self, position: FieldPosition) -> Result<NodeIndex, InternetError> {
		let id = self.network.node_count();
		let range = self.ip_range_iter.next().ok_or(InternetError::TooManyNetworks)?;

		let network = InternetNetwork::new(id, range);
		let node = InternetNode::from_network(network, position, id);
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