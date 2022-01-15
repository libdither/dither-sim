use std::{net::Ipv4Addr, time::Duration};

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
	Connected(Ipv4Addr),
}
pub struct InternetMachine {
	pub machine: Machine<DeviceCommand, DeviceEvent>,
	pub event_join_handle: JoinHandle<()>,
	pub connection_status: InternetMachineConnection,
	/// Wire for internal latency simulation
	pub internal_wire: WireHandle, 
	pub internal_latency: Latency,
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

		let (outgoing_plug, plug_from_wire) = netsim_embed::wire();
		let wire = Wire { delay: Duration::from_millis(DEFAULT_INTERNAL_LATENCY) };
		let internet_machine = InternetMachine {
			id: machine_id,
			machine,
			event_join_handle,
			connection_status: InternetMachineConnection::Unconnected(outgoing_plug),
			internal_wire: wire.connect(plug_to_wire, plug_from_wire),
			internal_latency: DEFAULT_INTERNAL_LATENCY,
		};
		internet_machine
	}
	pub async fn set_latency(&mut self, latency: Latency) {
		self.internal_latency = latency;
		self.internal_wire.set_delay(Duration::from_millis(self.internal_latency)).await;
	}
	pub fn latency(&self) -> Latency {
		self.internal_latency
	}
	pub fn request_machine_info(&self) -> Result<(), InternetError> {
		self.machine.tx.unbounded_send(DeviceCommand::DitherCommand(DitherCommand::GetNodeInfo)).map_err(|_|InternetError::DeviceCommandSenderClosed)
	}
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
	connections: Vec<(usize, bool)>,
}
impl InternetNetwork {
	pub fn new(id: usize, range: Ipv4Range) -> Self {
		Self { network: Network::new(NetworkId(id), range), connections: Default::default() }
	}
	pub fn local_addr(&self) -> Ipv4Addr { self.network.range().base_addr() }
	pub fn connect(&self, id: usize, plug: Plug, routes: Vec<Ipv4Route>) {
		self.network.router.add_connection(id, plug, routes);
	}
	pub fn network_info(&self) -> NetworkInfo {
		NetworkInfo {
			connections: self.connections.clone(),
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

enum NodeVariant {
	Network(InternetNetwork),
	Machine(InternetMachine),
}

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
pub struct InternetNode {
	variant: NodeVariant,
	/// Position of this Node in the Internet Simulation Field
	position: FieldPosition,
}

impl InternetNode {
	pub fn from_machine(machine: InternetMachine, position: FieldPosition) -> Self {
		Self {
			variant: NodeVariant::Machine(machine),
			position,
		}
	}
	pub fn from_network(network: InternetNetwork, position: FieldPosition) -> Self {
		Self {
			variant: NodeVariant::Network(network),
			position,
		}
	}
	pub fn node_info(&self) -> NodeInfo {
		let (internal_latency, local_address, node_type) = match &self.variant {
			NodeVariant::Network(network) => {
				(Latency::MIN, Some(network.local_addr()), NodeType::Network)
			},
			NodeVariant::Machine(machine) => {
				(machine.latency(), match machine.connection_status {
					InternetMachineConnection::Connected(addr) => Some(addr),
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
	pub fn machine(&self) -> Option<&InternetMachine> {
		match &self.variant { NodeVariant::Machine(m) => Some(m), _ => None }
	}
	pub fn network(&self) -> Option<&InternetNetwork> {
		match &self.variant { NodeVariant::Network(n) => Some(n), _ => None }
	}
}

