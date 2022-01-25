#![allow(dead_code)]

/// Internet Simulation Module
/// Contains all necessary componenets to create a virtual network on a given computer and spawn devices running the Dither protocol

use std::fmt::{self, Debug};
use std::fs::File;
use std::io::BufReader;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_std::task;
use futures::StreamExt;
use slotmap::{SecondaryMap, SlotMap, new_key_type};
use serde::Deserialize;
use futures::channel::mpsc;

use netsim_embed::Ipv4RangeIter;

use device::{DeviceCommand, DeviceEvent, DitherEvent};

mod netsim_ext;
mod internet_node;
use netsim_ext::*;

pub use internet_node::{FieldPosition, InternetNetwork, InternetMachine, InternetNode, NodeType, NodeInfo, MachineInfo, NetworkInfo, Latency, NodeVariant};

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
	GetNodeInfo(NodeIdx), // Get info about node
	/// Get info about a given Machine running Dither -> MachineInfo
	GetMachineInfo(NodeIdx), // Get info about machine
	/// Get info about a given Network thread -> NetworkInfo
	GetNetworkInfo(NodeIdx), // Get info about network
	//Send Dither-specific action to a machine?
	///SendMachineAction(usize),

	/// Change position of a given node in the network
	SetPosition(NodeIdx, FieldPosition),
	/// Connect two nodes
	ConnectNodes(NodeIdx, NodeIdx),

	/// Send Device command (Dither-specific or otherwise) -> NodeInfo & MachineInfo
	DeviceCommand(NodeIdx, DeviceCommand),

	// From Devices
	HandleDeviceEvent(NodeIdx, DeviceEvent)
}

/// Internet Simulation Events, use this structure to listen to events from the simulation thread
#[derive(Debug, Clone)]
pub enum InternetEvent {
	/// New machine was created
	NewMachine(NodeIdx),
	/// Net network was created
	NewNetwork(NodeIdx),
	/* /// Connection between two nodes created
	NewConnection(WireIdx), */
	/// General Node info 
	NodeInfo(NodeIdx, NodeInfo),
	/// General machine info
	MachineInfo(NodeIdx, MachineInfo),
	/// General network info
	NetworkInfo(NodeIdx, NetworkInfo),
	/// Connection Info
	ConnectionInfo(WireIdx, NodeIdx, NodeIdx), // Whether or not to activate / deactivate a connection between two nodes

	/// Error
	Error(Arc<InternetError>), // Must use Arc for clone misdirection since iced requires messages to be Clone
}

/// Internet Error object
#[derive(Error, Debug)]
pub enum InternetError {
	#[error("Event Receiver Closed")]
	EventReceiverClosed,
	#[error("Action Sender Closed")]
	ActionSenderClosed,
	#[error("No Runtime")]
	NoRuntime,

	#[error("Internet Machine Error: {0}")]
	InternetMachineError(#[from] internet_node::MachineError),
	#[error("Internet Network Error: {0}")]
	InternetNetworkError(#[from] internet_node::NetworkError),

	#[error("Invalid Node Type for {index}, expected: {expected:?}")]
	InvalidNodeType { index: NodeIdx, expected: NodeType },
	#[error("Unknown Node index: {index}")]
	UnknownNode { index: NodeIdx },
	#[error("Unknown Wire index: {index}")]
	UnknownWire { index: WireIdx },
	#[error("Can't connect machines directory to each other")]
	NodeConnectionError,

	#[error("Spawned Too many networks, not enough addresses (see MAX_NETWORKS)")]
	TooManyNetworks,

	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

new_key_type! { pub struct NodeIdx; }
impl fmt::Display for NodeIdx { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self) } }
impl NodeIdx { pub fn as_usize(&self) -> usize { self.0.as_ffi() as usize } }

new_key_type! { pub struct WireIdx; }
impl fmt::Display for WireIdx { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self) } }
impl WireIdx { pub fn as_usize(&self) -> usize { self.0.as_ffi() as usize } }

/// Internet object, contains handles to the network and machine threads
#[derive(Serialize, Deserialize)]
pub struct Internet {
	nodes: SlotMap<NodeIdx, InternetNode>,
	wires: SlotMap<WireIdx, (NodeIdx, NodeIdx)>,
	device_exec: String,
	ip_range_iter: Ipv4RangeIter,

	#[serde(skip)]
	runtime: Option<InternetRuntime>,
}

pub struct InternetRuntime {
	wire_handles: SecondaryMap<WireIdx, WireHandle>,

	action_receiver: Option<mpsc::Receiver<InternetAction>>,
	action_sender: mpsc::Sender<InternetAction>,
	pub event_sender: mpsc::Sender<InternetEvent>,
}

impl Internet {
	/// Create new internet instance with action senders and event receivers
	pub fn new(device_exec: impl Into<String>) -> Internet {
		Internet {
			nodes: SlotMap::default(),
			wires: SlotMap::default(),
			runtime: None,
			device_exec: device_exec.into(),
			ip_range_iter: Ipv4RangeIter::new(MAX_NETWORKS as u32),
		}
	}
	pub fn save(&self, filepath: &str) -> Result<(), InternetError> {
		use std::io::Write;
		let mut file = File::create(filepath).context("failed to create file (check perms) at {}")?;
		let data = bincode::serialize(&self).context("failed to serialize network")?;
		file.write_all(&data).context("failed to write to file")?;
		Ok(())
	}
	pub fn load(filepath: &str) -> Result<Self, InternetError> {
		let file = File::open(filepath).context("failed to open file (check perms)")?;
		let internet: Internet = bincode::deserialize_from(BufReader::new(file)).context("failed to deserialize network")?;
		Ok(internet)
	}
	fn node(&self, idx: NodeIdx) -> Result<&InternetNode, InternetError> {
		self.nodes.get(idx).ok_or(InternetError::UnknownNode { index: idx })
	}
	fn node_mut(&mut self, idx: NodeIdx) -> Result<&mut InternetNode, InternetError> {
		self.nodes.get_mut(idx).ok_or(InternetError::UnknownNode { index: idx })
	} 
	pub fn machine(&self, index: NodeIdx) -> Result<&InternetMachine, InternetError> {
		self.node(index)?.machine().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Machine })
	}
	pub fn machine_mut(&mut self, index: NodeIdx) -> Result<&mut InternetMachine, InternetError> {
		self.node_mut(index)?.machine_mut().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Machine })
	}
	pub fn network(&self, index: NodeIdx) -> Result<&InternetNetwork, InternetError> {
		self.node(index)?.network().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Network })
	}
	pub fn network_mut(&mut self, index: NodeIdx) -> Result<&mut InternetNetwork, InternetError> {
		self.node_mut(index)?.network_mut().ok_or(InternetError::InvalidNodeType { index, expected: NodeType::Network })
	}

	pub async fn init(&self) -> (InternetRuntime, mpsc::Receiver<InternetEvent>, mpsc::Sender<InternetAction>) {
		let (event_sender, event_receiver) = mpsc::channel(20);
		let (action_sender, action_receiver) = mpsc::channel(20);

		let action_sender_ret = action_sender.clone();
		(InternetRuntime {
			wire_handles: SecondaryMap::default(),
			action_receiver: Some(action_receiver),
			action_sender,
			event_sender,
		}, event_receiver, action_sender_ret)
	}
	/// Run network function
	/// IMPORTANT: This function must be called from an unshare() context (i.e. a kernel virtual network)
	pub async fn run(mut self, mut runtime: InternetRuntime) {
		std::fs::metadata(&self.device_exec).expect("no device file!");

		let mut action_receiver = runtime.action_receiver.take().expect("there should be an action receiver here");
		self.runtime = Some(runtime);
		while let Some(action) = action_receiver.next().await {
			let res: Result<(), InternetError> = try {
				println!("Received Internet Action: {:?}", action);
				match action {
					InternetAction::AddNetwork(position) => {
						let idx = self.spawn_network(position)?;
						self.send_event(InternetEvent::NewNetwork(idx))?;
						self.action(InternetAction::GetNodeInfo(idx))?;
						self.action(InternetAction::GetNetworkInfo(idx))?;
						log::debug!("Added Network Node: {:?}", idx);
					}
					InternetAction::AddMachine(position) => {
						let idx = self.spawn_machine(position)?;
						self.send_event(InternetEvent::NewMachine(idx))?;
						self.action(InternetAction::GetNodeInfo(idx))?;
						self.action(InternetAction::GetMachineInfo(idx))?;
						log::debug!("Added Machine Node: {:?}", idx);
					}
					InternetAction::ConnectNodes(from, to) => {
						use NodeVariant::*;
						let node1 = self.node(from)?;
						let node2 = self.node(to)?;
						match (&node1.variant, &node2.variant) {
							(Network(net1), Network(net2)) => {
								let delay = Duration::from_micros(node1.latency_distance(node2));
								let (route1, route2) = (net1.route(), net2.route());
								
								let wire_idx = self.wires.insert((from.min(to), from.max(to)));

								let plug1 = self.network_mut(from)?.connect(wire_idx, to, vec![route1])?;
								let plug2 = self.network_mut(to)?.connect(wire_idx, from, vec![route2])?;
								self.runtime()?.wire_handles.insert(wire_idx, Wire { delay }.connect(plug1, plug2));
								self.send_event(InternetEvent::ConnectionInfo(wire_idx, from, to))?;
								self.action(InternetAction::GetNodeInfo(from))?;
								self.action(InternetAction::GetNodeInfo(to))?;
								self.action(InternetAction::GetNetworkInfo(from))?;
								self.action(InternetAction::GetNetworkInfo(to))?;
							},
							(Network(net), Machine(machine)) | (Machine(machine), Network(net)) => {
								let machine_id = machine.id; let net_id = net.id;
								let wire_idx = self.plug(machine_id, net_id).await?;
								self.send_event(InternetEvent::ConnectionInfo(wire_idx, net_id, machine_id))?;
								self.action(InternetAction::GetNodeInfo(net_id))?;
								self.action(InternetAction::GetNetworkInfo(net_id))?;
								self.action(InternetAction::GetNodeInfo(machine_id))?;
								self.action(InternetAction::GetMachineInfo(machine_id))?;
							}
							_ => Err(InternetError::NodeConnectionError)?,
						}
					}
					InternetAction::SetPosition(_node, _position) => {
						//self.machine(node)?.update_position(position)?;
					}
					InternetAction::GetNodeInfo(index) => {
						self.send_event(InternetEvent::NodeInfo(index, self.node(index)?.node_info()))?;
					}
					InternetAction::GetMachineInfo(index) => {
						// This is sent back from the Device through DeviceEvents
						self.machine_mut(index)?.request_machine_info()?;
					}
					InternetAction::GetNetworkInfo(index) => {
						self.send_event(InternetEvent::NetworkInfo(index, self.network(index)?.network_info()))?;

					}
					InternetAction::HandleDeviceEvent(index, DeviceEvent::DitherEvent(dither_event)) => {
						match dither_event {
							DitherEvent::NodeInfo(device::NodeInfo { route_coord, node_id, public_addr, remotes, active_remotes } ) => {
								//let machine = self.machine(index)?;
								self.send_event(InternetEvent::MachineInfo(index, MachineInfo {
									route_coord, public_addr, node_id, remotes, active_remotes
								}))?;
							}
							//_ => log::error!("Unhandled Device Event")
						}
					}
					_ => log::error!("Unimplemented Internet Action")
				}
			};
			if let Err(err) = res {
				if let Err(_) = self.send_event(InternetEvent::Error(Arc::new(err))) {
					log::error!("InternetEvent receiver closed unexpectedly");
					break;
				}
			}
		}
	}
	fn runtime(&mut self) -> Result<&mut InternetRuntime, InternetError> {
		self.runtime.as_mut().ok_or(InternetError::NoRuntime)
	}
	/// Send event function (used internally by run())
	fn send_event(&mut self, event: InternetEvent) -> Result<(), InternetError> {
		self.runtime()?.event_sender.try_send(event).map_err(|_|InternetError::EventReceiverClosed)
	}
	fn action(&mut self, action: InternetAction) -> Result<(), InternetError> {
		self.runtime()?.action_sender.try_send(action).map_err(|_|InternetError::ActionSenderClosed)
	}
	/// Spawn machine at position
	fn spawn_machine(&mut self, position: FieldPosition) -> Result<NodeIdx, InternetError> {
		let action_sender = self.runtime()?.action_sender.clone();
		let executable = self.device_exec.clone();
		Ok(self.nodes.insert_with_key(|key| {
			let mut machine = task::block_on(InternetMachine::new(key, executable));
			machine.init(action_sender);
			InternetNode::from_machine(machine, position, key)
		}))
	}
	/// Spawn network at position
	fn spawn_network(&mut self, position: FieldPosition) -> Result<NodeIdx, InternetError> {
		let range = self.ip_range_iter.next().ok_or(InternetError::TooManyNetworks)?;
		Ok(self.nodes.insert_with_key(|key|{
			let mut network = InternetNetwork::new(key, range);
			network.init();
			InternetNode::from_network(network, position, key)
		}))
	}
	async fn plug(&mut self, machine_id: NodeIdx, network_id: NodeIdx) -> Result<WireIdx, InternetError> {
		// Disconnect if connected
		if let Some((wire_idx, _)) = self.machine(machine_id)?.connection {
			self.unwire(wire_idx)?;
		}

		// Connect
		let wire_idx = self.wires.insert((machine_id, network_id));
		let network = self.network_mut(network_id)?;
		let addr = network.unique_addr();
		let net_plug = network.connect(wire_idx, machine_id, vec![addr.into()])?;
		let machine_plug = self.machine_mut(machine_id)?.connect(wire_idx, addr).await?;
		let delay = Duration::from_micros(self.node(machine_id)?.latency_distance(self.node(network_id)?));

		//let delay = self.node(machine_id)?.position
		self.runtime()?.wire_handles.insert(wire_idx, Wire::connect(Wire { delay }, net_plug, machine_plug));
		Ok(wire_idx)
	}
	fn unwire(&mut self, wire_idx: WireIdx) -> Result<(), InternetError> {
		self.runtime()?.wire_handles.remove(wire_idx);
		if let Some((node1, node2)) = self.wires.remove(wire_idx) {
			self.node_mut(node1)?.disconnect(wire_idx)?;
			self.node_mut(node2)?.disconnect(wire_idx)?;
		}
		Ok(())
	}
}