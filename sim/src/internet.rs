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

use device::{Address, DeviceCommand, DeviceEvent, DitherCommand, DitherEvent};
pub use node::{self, RouteCoord, NodeID, net};

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
	/// Save network to path given by string
	SaveInternet(String),
	/// Request all info
	RequestAllNodes,
	ConnectAllMachines(NodeIdx), // Send connection requests from all machines to given NodeIdx to organize network
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
	GetConnectionInfo(WireIdx),
	///SendMachineAction(usize),

	/// Change position of a given node in the network
	SetPosition(NodeIdx, FieldPosition),
	/// Connect two nodes
	ConnectNodes(NodeIdx, NodeIdx),

	/// Send Device command (Dither-specific or otherwise)
	DeviceCommand(NodeIdx, DeviceCommand),
	/// Send DitherCommand to device
	DitherCommand(NodeIdx, DitherCommand),
	/// Fetch global ip from network configuration and pass it to the device so that there is at least one node that can be bootstrapped off of.
	TellIp(NodeIdx),

	// From Devices
	HandleDeviceEvent(NodeIdx, DeviceEvent),
	DebugPrint,
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
	RemoveConnection(WireIdx),

	/// Reset 
	ClearUI,

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
impl NodeIdx {
	pub fn as_ffi(&self) -> usize { self.0.as_ffi() as usize }
}

new_key_type! { pub struct WireIdx; }
impl fmt::Display for WireIdx { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self) } }
impl WireIdx { pub fn as_ffi(&self) -> usize { self.0.as_ffi() as usize } }

/// Internet object, contains handles to the network and machine threads
#[derive(Serialize, Deserialize, Debug)]
pub struct Internet {
	nodes: SlotMap<NodeIdx, InternetNode>,
	wires: SlotMap<WireIdx, (NodeIdx, NodeIdx)>,
	device_exec: String,
	ip_range_iter: Ipv4RangeIter,
}

pub struct InternetRuntime {
	node_locations: SecondaryMap<NodeIdx, FieldPosition>,
	wire_handles: SecondaryMap<WireIdx, WireHandle>,

	action_receiver: Option<mpsc::Receiver<InternetAction>>,
	action_sender: mpsc::Sender<InternetAction>,
	pub event_sender: mpsc::Sender<InternetEvent>,
}

impl InternetRuntime {
	/// Send event function (used internally by run())
	fn send_event(&mut self, event: InternetEvent) -> Result<(), InternetError> {
		self.event_sender.try_send(event).map_err(|_|InternetError::EventReceiverClosed)
	}
	fn action(&mut self, action: InternetAction) -> Result<(), InternetError> {
		self.action_sender.try_send(action).map_err(|_|InternetError::ActionSenderClosed)
	}
	fn location(&mut self, index: NodeIdx) -> Result<&mut FieldPosition, InternetError> {
		self.node_locations.get_mut(index).ok_or(InternetError::UnknownNode { index })
	}
	fn wire_handle(&mut self, wire_idx: WireIdx) -> Result<&mut WireHandle, InternetError> {
		self.wire_handles.get_mut(wire_idx).ok_or(InternetError::UnknownWire { index: wire_idx })
	}
}

impl Internet {
	/// Create new internet instance with action senders and event receivers
	pub fn new(device_exec: impl Into<String>) -> Internet {
		Internet {
			nodes: SlotMap::default(),
			wires: SlotMap::default(),
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
		log::debug!("Loading Internet from: {:?}", filepath);
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

	pub async fn init(&mut self) -> Result<(InternetRuntime, mpsc::Receiver<InternetEvent>, mpsc::Sender<InternetAction>), InternetError> {
		let (event_sender, event_receiver) = mpsc::channel(100);
		let (action_sender, action_receiver) = mpsc::channel(100);

		let action_sender_ret = action_sender.clone();

		let mut runtime = InternetRuntime {
			node_locations: SecondaryMap::default(),
			wire_handles: SecondaryMap::default(),
			action_receiver: Some(action_receiver),
			action_sender,
			event_sender,
		};
		// Init Nodes
		for (node_idx, node) in self.nodes.iter_mut() {
			runtime.node_locations.insert(node_idx, node.position.clone());
			match &mut node.variant {
				NodeVariant::Machine(machine) => {
					machine.init(action_sender_ret.clone());
				}
				NodeVariant::Network(network) => {
					network.init();
				}
			}
		}

		// Init Wire Handles
		for (wire_idx, (node1, node2)) in self.wires.clone().into_iter() {
			log::debug!("wire: {} connecting {} and {}", wire_idx, node1, node2);
			let delay = Duration::from_micros(InternetNode::latency_distance(&self.node(node1)?.position, &self.node(node2)?.position));
			let plug_a = self.node_mut(node1)?.init_plug(wire_idx)?;
			let plug_b = self.node_mut(node2)?.init_plug(wire_idx)?;
			runtime.wire_handles.insert(wire_idx, Wire::connect(Wire { delay }, plug_a, plug_b));
		}
		if self.nodes.len() > 0 {
			runtime.action(InternetAction::RequestAllNodes)?;
		}
		

		Ok((runtime, event_receiver, action_sender_ret))
	}
	/// Run network function
	/// IMPORTANT: This function must be called from an unshare() context (i.e. a kernel virtual network)
	pub async fn run(mut self, mut runtime: InternetRuntime) {
		std::fs::metadata(&self.device_exec).expect("no device file!");
		let runtime = &mut runtime;

		let mut action_receiver = runtime.action_receiver.take().expect("there should be an action receiver here");
		while let Some(action) = action_receiver.next().await {
			let res: Result<(), InternetError> = try {
				log::debug!("Received InternetAction: {:?}", action);
				match action {
					InternetAction::SaveInternet(location) => {
						self.save(&location)?;
						log::debug!("Saved Network");
					}
					InternetAction::RequestAllNodes => {
						runtime.send_event(InternetEvent::ClearUI)?;
						for (idx, node) in self.nodes.iter() {
							let node_info = node.node_info();
							runtime.send_event(InternetEvent::NodeInfo(idx, node_info))?;
							match &self.nodes[idx].variant {
								NodeVariant::Machine(machine) => machine.request_machine_info()?,
								NodeVariant::Network(network) => runtime.send_event(InternetEvent::NetworkInfo(idx, network.network_info()))?,
							}
						}
						for (wire_idx, &(node1, node2)) in self.wires.iter() {
							runtime.send_event(InternetEvent::ConnectionInfo(wire_idx, node1, node2))?;
						}
					}
					/* InternetAction::ConnectAllMachines(node_idx) => {
						self.machine(node_idx)?.
					} */
					InternetAction::AddNetwork(position) => {
						let idx = self.spawn_network(runtime, position)?;
						runtime.send_event(InternetEvent::NewNetwork(idx))?;
						runtime.action(InternetAction::GetNodeInfo(idx))?;
						runtime.action(InternetAction::GetNetworkInfo(idx))?;
						log::debug!("Added Network Node: {:?}", idx);
					}
					InternetAction::AddMachine(position) => {
						let idx = self.spawn_machine(runtime, position)?;
						runtime.send_event(InternetEvent::NewMachine(idx))?;
						runtime.action(InternetAction::GetNodeInfo(idx))?;
						runtime.action(InternetAction::GetMachineInfo(idx))?;
						log::debug!("Added Machine Node: {:?}", idx);
					}
					InternetAction::ConnectNodes(from, to) => {
						let wire_idx = self.connect(runtime, from, to).await?;
						runtime.send_event(InternetEvent::ConnectionInfo(wire_idx, from, to))?;
					}
					InternetAction::SetPosition(index, position) => {
						let node = self.node_mut(index)?;
						node.update_position(runtime, position).await?;
						runtime.send_event(InternetEvent::NodeInfo(index, node.node_info()))?;
					}
					InternetAction::GetNodeInfo(index) => {
						runtime.send_event(InternetEvent::NodeInfo(index, self.node(index)?.node_info()))?;
					}
					InternetAction::GetMachineInfo(index) => {
						// This is sent back from the Device through DeviceEvents
						self.machine_mut(index)?.request_machine_info()?;
					}
					InternetAction::GetNetworkInfo(index) => {
						runtime.send_event(InternetEvent::NetworkInfo(index, self.network(index)?.network_info()))?;
					}
					InternetAction::GetConnectionInfo(wire_idx) => {
						let (from, to) = self.wires.get(wire_idx).cloned().ok_or(InternetError::UnknownWire { index: wire_idx })?;
						runtime.send_event(InternetEvent::ConnectionInfo(wire_idx, from, to))?;
					}
					InternetAction::HandleDeviceEvent(index, DeviceEvent::DitherEvent(dither_event)) => {
						match dither_event {
							DitherEvent::NodeInfo(device::NodeInfo { route_coord, node_id, public_addr, remotes, active_remotes } ) => {
								let network_ip = self.machine(index)?.connection_ip();
								runtime.send_event(InternetEvent::MachineInfo(index, MachineInfo {
									route_coord, public_addr, node_id, remotes, active_remotes, network_ip,
								}))?;
							}
							//_ => log::error!("Unhandled Device Event")
						}
					}
					InternetAction::DeviceCommand(node_idx, command) => {
						self.machine(node_idx)?.device_command(command)?;
					}
					InternetAction::DitherCommand(node_idx, command) => {
						self.machine(node_idx)?.device_command(DeviceCommand::DitherCommand(command))?;
					}
					/* InternetAction::TellIp(node_idx) => {
						let machine = self.machine(node_idx)?;
						if let Some(ip) = machine.connection_ip() {
							machine.device_command(DeviceCommand::DitherCommand(DitherCommand::SetPublicIp(ip)))?;
						} else { log::error!("Cannot tell {:?} its own ip as it is not connected to any network", node_idx); }

					} */
					InternetAction::DebugPrint => {
						log::debug!("Internet State: {:#?}", &self);
					}
					_ => log::error!("Unimplemented Internet Action")
				}
			};
			if let Err(err) = res {
				if let Err(err) = runtime.send_event(InternetEvent::Error(Arc::new(err))) {
					log::error!("InternetEvent receiver closed unexpectedly when sending error: {:?}", err);
					break;
				}
			}
		}
	}
	/// Spawn machine at position
	fn spawn_machine(&mut self, runtime: &mut InternetRuntime, position: FieldPosition) -> Result<NodeIdx, InternetError> {
		let action_sender = runtime.action_sender.clone();
		let executable = self.device_exec.clone();
		Ok(self.nodes.insert_with_key(|key| {
			let mut machine = task::block_on(InternetMachine::new(key, executable));
			machine.init(action_sender);
			InternetNode::from_machine(machine, position, key)
		}))
	}
	/// Spawn network at position
	fn spawn_network(&mut self, _runtime: &mut InternetRuntime, position: FieldPosition) -> Result<NodeIdx, InternetError> {
		let range = self.ip_range_iter.next().ok_or(InternetError::TooManyNetworks)?;
		Ok(self.nodes.insert_with_key(|key|{
			let mut network = InternetNetwork::new(key, range);
			network.init();
			InternetNode::from_network(network, position, key)
		}))
	}
	async fn connect(&mut self, runtime: &mut InternetRuntime, from: NodeIdx, to: NodeIdx) -> Result<WireIdx, InternetError> {
		use NodeVariant::*;
		let node1 = self.node(from)?;
		let node2 = self.node(to)?;
		match (&node1.variant, &node2.variant) {
			(Network(net1), Network(net2)) => {
				let delay = Duration::from_micros(InternetNode::latency_distance(&node1.position, &node2.position));
				let (route1, route2) = (net1.route(), net2.route());

				let wire_idx = self.wires.insert((from, to));
				
				let plug1 = self.network_mut(from)?.connect(wire_idx, to, vec![route1])?;
				let plug2 = self.network_mut(to)?.connect(wire_idx, from, vec![route2])?;
				runtime.wire_handles.insert(wire_idx, Wire { delay }.connect(plug1, plug2));
				Ok(wire_idx)
			},
			(Network(net), Machine(machine)) | (Machine(machine), Network(net)) => {
				let machine_id = machine.id; let network_id = net.id;
				// Disconnect if connected
				if let Some((wire_idx, _, _)) = self.machine(machine_id)?.connection {
					self.unwire(runtime, wire_idx)?;
				}

				let wire_idx = self.wires.insert((from, to));

				// Connect
				let network = self.network_mut(network_id)?;
				let addr = network.unique_addr();
				let net_plug = network.connect(wire_idx, machine_id, vec![addr.into()])?;
				let machine_plug = self.machine_mut(machine_id)?.connect(wire_idx, network_id, addr).await?;
				let delay = Duration::from_micros(InternetNode::latency_distance(&self.node(machine_id)?.position, &self.node(network_id)?.position));

				//let delay = self.node(machine_id)?.position
				runtime.wire_handles.insert(wire_idx, Wire::connect(Wire { delay }, net_plug, machine_plug));
				Ok(wire_idx)
			}
			_ => Err(InternetError::NodeConnectionError),
		}
	}
	fn unwire(&mut self, runtime: &mut InternetRuntime, wire_idx: WireIdx) -> Result<(), InternetError> {
		runtime.wire_handles.remove(wire_idx);
		if let Some((node1, node2)) = self.wires.remove(wire_idx) {
			runtime.send_event(InternetEvent::RemoveConnection(wire_idx))?;
			self.node_mut(node1)?.disconnect(wire_idx)?;
			self.node_mut(node2)?.disconnect(wire_idx)?;
		}
		Ok(())
	}
}