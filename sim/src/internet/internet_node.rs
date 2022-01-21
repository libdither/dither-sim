use std::{collections::HashMap, net::Ipv4Addr, sync::Arc, time::Duration};

use async_std::task::{self, JoinHandle};
use device::{Address, DeviceCommand, DeviceEvent, DitherCommand, DitherEvent};
use futures::{SinkExt, StreamExt, channel::mpsc};
use nalgebra::Vector2;
use netsim_embed::{Ipv4Range, Ipv4Route, Machine, MachineId, Network, NetworkId, Plug};
use node::{NodeID, RouteCoord};

use crate::internet::{InternetAction, InternetError};

use super::netsim_ext::{Wire, WireHandle};

pub type FieldPosition = Vector2<i32>;
/// Measured in milliseconds
pub type Latency = u64;

/// Default internal latency (measured in millilightseconds)
pub const DEFAULT_INTERNAL_LATENCY: Latency = 20;

pub enum InternetMachineConnection {
	Unconnected(Plug),
	Connected(usize, Ipv4Addr),
}
pub struct InternetMachine {
	pub machine: Machine<DeviceCommand, DeviceEvent>,
	pub event_join_handle: JoinHandle<()>,
	pub connection_status: InternetMachineConnection,
	/// Wire for internal latency simulation
	pub internal_wire_handle: WireHandle, 
	pub internal_latency: Latency,
	pub outgoing_wire_handle: Arc<WireHandle>,
	pub id: usize,
}
impl InternetMachine {
	pub async fn new(machine_id: usize, mut internet_action_sender: mpsc::Sender<InternetAction>, executable: impl AsRef<std::ffi::OsStr>) -> Self {
		let (plug_to_wire, machine_plug) = netsim_embed::wire();
		
		let (machine, mut device_event_receiver) = Machine::new(MachineId(machine_id), machine_plug, async_process::Command::new(executable)).await;
		//let mut action_sender = self.action_sender.clone();
		let event_join_handle = task::spawn(async move { 
			while let Some(device_event) = device_event_receiver.next().await {
				internet_action_sender.send(InternetAction::HandleDeviceEvent(machine_id, device_event)).await.expect("device action sender crashed");
			}
		});

		let (outgoing_plug, plug_from_wire, outgoing_wire_handle) = Wire::new(Duration::ZERO);
		let internal_wire = Wire { delay: Duration::from_millis(DEFAULT_INTERNAL_LATENCY) };
		let internet_machine = InternetMachine {
			id: machine_id,
			machine,
			event_join_handle,
			connection_status: InternetMachineConnection::Unconnected(outgoing_plug),
			outgoing_wire_handle,
			internal_wire_handle: internal_wire.connect(plug_to_wire, plug_from_wire),
			internal_latency: DEFAULT_INTERNAL_LATENCY,
		};
		internet_machine
	}
	pub async fn set_latency(&mut self, latency: Latency) {
		self.internal_latency = latency;
		self.internal_wire_handle.set_delay(Duration::from_millis(self.internal_latency)).await;
	}
	pub fn latency(&self) -> Latency {
		self.internal_latency
	}
	pub fn request_machine_info(&self) -> Result<(), InternetError> {
		self.machine.tx.unbounded_send(DeviceCommand::DitherCommand(DitherCommand::GetNodeInfo)).map_err(|_|InternetError::DeviceCommandSenderClosed)
	}
	/* pub async fn update_position(&mut self) -> Result<(), InternetError> {
		match self.connection_status {
			InternetMachineConnection::
		}
	} */
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
	pub position: FieldPosition,
	pub internal_latency: Latency,
	pub local_address: Option<Ipv4Addr>,
	pub node_type: NodeType,
}
#[derive(Debug, Clone)]
pub struct MachineInfo {
	pub route_coord: Option<RouteCoord>,
	pub public_addr: Option<Address>,
	pub node_id: NodeID,
	pub remotes: usize,
	pub active_remotes: usize,
}

pub struct InternetNetwork {
	network: Network,
	connections: HashMap<usize, (bool, Arc<WireHandle>)>,
}
impl InternetNetwork {
	pub fn new(id: usize, range: Ipv4Range) -> Self {
		Self { network: Network::new(NetworkId(id), range), connections: Default::default() }
	}
	pub fn unique_id(&self) -> usize { self.network.id().0 }
	pub fn local_addr(&self) -> Ipv4Addr { self.network.range().base_addr() }

	pub fn route(&self) -> Ipv4Route { self.network.range().into() }
	pub fn unique_addr(&mut self) -> Ipv4Addr {
		self.network.unique_addr()
	}
	pub fn connect(&mut self, id: usize, plug: Plug, routes: Vec<Ipv4Route>, wire_handle: Arc<WireHandle>) {
		self.connections.insert(id, (true, wire_handle));
		self.network.router.add_connection(id, plug, routes);
	}
	pub async fn disconnect(&mut self, id: usize) -> Plug {
		self.connections.remove(&id);
		self.network.router.remove_connection(id).await.expect("There should be plug here")
	}
	pub fn network_info(&self) -> NetworkInfo {
		NetworkInfo {
			connections: self.connections.iter().map(|(id, (active, _))|(*id, *active)).collect(),
			ip_range: self.network.range().clone()
		}
	}
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
	pub connections: Vec<(usize, bool)>,
	pub ip_range: Ipv4Range,
}

#[derive(Debug, Clone)]
pub enum NodeType {
	Network,
	Machine,
}

pub enum NodeVariant {
	Network(InternetNetwork),
	Machine(InternetMachine),
}

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
pub struct InternetNode {
	pub variant: NodeVariant,
	/// Position of this Node in the Internet Simulation Field
	position: FieldPosition,
	/// Index of Node in Internet.map
	id: usize,
}

impl InternetNode {
	pub fn from_machine(machine: InternetMachine, position: FieldPosition, id: usize) -> Self {
		Self {
			variant: NodeVariant::Machine(machine),
			position, id,
		}
	}
	pub fn from_network(network: InternetNetwork, position: FieldPosition, id: usize) -> Self {
		Self {
			variant: NodeVariant::Network(network),
			position, id,
		}
	}
	pub fn node_info(&self) -> NodeInfo {
		let (internal_latency, local_address, node_type) = match &self.variant {
			NodeVariant::Network(network) => {
				(Latency::MIN, Some(network.local_addr()), NodeType::Network)
			},
			NodeVariant::Machine(machine) => {
				(machine.latency(), match machine.connection_status {
					InternetMachineConnection::Connected(_net_id, addr) => Some(addr),
					InternetMachineConnection::Unconnected(_) => None,
				}, NodeType::Machine)
			},
		};
		NodeInfo {
			position: self.position.clone(),
			internal_latency,
			local_address,
			node_type,
		}
	}
	pub fn latency_distance(&self, other: &Self) -> Latency {
		self.position.map(|v|v as f64).metric_distance(&other.position.map(|v|v as f64)) as Latency
	}
	pub fn machine(&self) -> Option<&InternetMachine> {
		match &self.variant { NodeVariant::Machine(m) => Some(m), _ => None }
	}
	pub fn machine_mut(&mut self) -> Option<&mut InternetMachine> {
		match &mut self.variant { NodeVariant::Machine(m) => Some(m), _ => None }
	}
	pub fn network(&self) -> Option<&InternetNetwork> {
		match &self.variant { NodeVariant::Network(n) => Some(n), _ => None }
	}
	pub fn network_mut(&mut self) -> Option<&mut InternetNetwork> {
		match &mut self.variant { NodeVariant::Network(n) => Some(n), _ => None }
	}
}

