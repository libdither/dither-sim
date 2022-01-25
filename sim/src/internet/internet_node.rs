use std::{net::Ipv4Addr, time::Duration};

use async_std::task::{self, JoinHandle};
use device::{Address, DeviceCommand, DeviceEvent, DitherCommand, DitherEvent};
use futures::{SinkExt, StreamExt, channel::mpsc};
use nalgebra::Vector2;
use netsim_embed::{Ipv4Range, Ipv4Route, Ipv4Router, Machine, MachineId, Plug};
use node::{NodeID, RouteCoord};
use slotmap::SecondaryMap;

use crate::internet::{InternetAction, InternetError, NodeIdx, WireIdx};

use super::netsim_ext::{Wire, WireHandle};

pub type FieldPosition = Vector2<i32>;
/// Measured in milliseconds
pub type Latency = u64;

/// Default internal latency (measured in millilightseconds)
pub const DEFAULT_INTERNAL_LATENCY: Latency = 20;

pub enum MachineConnection {
	Unconnected,
	Connected(WireIdx, Ipv4Addr),
}
#[derive(Serialize, Deserialize)]
pub struct InternetMachine {
	pub id: NodeIdx,
	internal_latency: Latency,
	executable: String,
	pub save_path: Option<String>,
	pub connection: Option<(WireIdx, Ipv4Addr)>,
	#[serde(skip)]
	runtime: Option<MachineRuntime>,
}
struct MachineRuntime {
	machine: Machine<DeviceCommand, DeviceEvent>,
	event_join_handle: JoinHandle<()>,
	internal_wire_handle: WireHandle,
}
#[derive(Debug, Error)]
pub enum MachineError {
	#[error("No runtime")]
	NoRuntime,
	#[error("Already Connected")]
	AlreadyConnected,
	#[error("Already Disconnected")]
	AlreadyDisconnected,
	#[error("Device Command Sender Closed")]
	DeviceCommandSenderClosed,
}

impl InternetMachine {
	pub async fn new(machine_id: NodeIdx, executable: String) -> Self {
		InternetMachine {
			id: machine_id,
			internal_latency: DEFAULT_INTERNAL_LATENCY,
			executable,
			save_path: None,
			connection: None,
			runtime: None,
		}
	}
	pub fn init(&mut self, mut internet_action_sender: mpsc::Sender<InternetAction>) {
		task::block_on(async move {
			let (machine_internal_plug, netsim_machine_plug) = netsim_embed::wire();

			let (machine, mut device_event_receiver)
			 = Machine::new(MachineId(self.id.as_usize()), netsim_machine_plug, async_process::Command::new(self.executable.clone())).await;

			let machine_id = self.id;
			let event_join_handle = task::spawn(async move { 
				while let Some(device_event) = device_event_receiver.next().await {
					if let Err(err) = internet_action_sender.send(InternetAction::HandleDeviceEvent(machine_id, device_event)).await {
						log::error!("Internet Action Sender closed: {:?}", err); break;
					}
				}
			});
	
			let (_, outgoing_internal_plug) = netsim_embed::wire();
			let internal_wire_handle = Wire { delay: Duration::from_micros(self.internal_latency) }.connect(outgoing_internal_plug, machine_internal_plug);
			self.runtime = Some(MachineRuntime {
				machine,
				event_join_handle,
				internal_wire_handle,
			});
		})
	}
	pub fn latency(&self) -> Latency {
		self.internal_latency
	}

	fn runtime(&mut self) -> Result<&mut MachineRuntime, MachineError> {
		self.runtime.as_mut().ok_or(MachineError::NoRuntime)
	}
	pub async fn set_latency(&mut self, latency: Latency) {
		self.internal_latency = latency;
		if let Some(runtime) = &mut self.runtime { 
			runtime.internal_wire_handle.set_delay(Duration::from_millis(self.internal_latency)).await;
		}
	}

	pub fn request_machine_info(&mut self) -> Result<(), MachineError> {
		self.runtime()?.machine.tx.unbounded_send(DeviceCommand::DitherCommand(DitherCommand::GetNodeInfo)).map_err(|_|MachineError::DeviceCommandSenderClosed)
	}

	pub async fn connect(&mut self, wire_idx: WireIdx, ip_addr: Ipv4Addr) -> Result<Plug, MachineError> {
		if let None = self.connection {
			let (outgoing_plug, outgoing_internal_plug) = netsim_embed::wire();
			self.runtime()?.internal_wire_handle.swap_plug_a(outgoing_internal_plug).await;
			self.connection = Some((wire_idx, ip_addr));
			Ok(outgoing_plug)
		} else { Err(MachineError::AlreadyConnected) }
	}
	/// Returns the wire connection
	pub fn connection(&mut self) -> Option<WireIdx> {
		if let Some((wire_idx, _)) = self.connection { Some(wire_idx) } else { None }
	}
	pub fn disconnect(&mut self) -> Result<(), MachineError> {
		if self.connection.is_some() { self.connection = None; Ok(()) }
		else { Err(MachineError::AlreadyDisconnected) }
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

#[derive(Serialize, Deserialize)]
pub struct InternetNetwork {
	pub id: NodeIdx,
    range: Ipv4Range,
    devices: u32,
	connections: SecondaryMap<WireIdx, (NodeIdx, Vec<Ipv4Route>)>,
	#[serde(skip)]
	runtime: Option<NetworkRuntime>,
}
pub struct NetworkRuntime {
    router: Ipv4Router,
}
#[derive(Debug, Error)]
pub enum NetworkError {
	#[error("No Runtime")]
	NoRuntime,
	#[error("Plug was not returned from Ipv4Router")]
	NoReturnedPlug,
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
	pub ip_range: Ipv4Range,
	pub connections: Vec<NodeIdx>,
}

impl InternetNetwork {
	pub fn new(id: NodeIdx, range: Ipv4Range) -> Self {
		Self {
			id, range, devices: 0,
			connections: SecondaryMap::<WireIdx, (NodeIdx, Vec<Ipv4Route>)>::default(),
			runtime: None,
		}
	}
	pub fn init(&mut self) {
		let router = Ipv4Router::new(self.range.gateway_addr());
		self.runtime = Some(NetworkRuntime { router })
		// TODO: Initialize router connections
	}
	pub fn id(&self) -> NodeIdx { self.id }
	pub fn local_addr(&self) -> Ipv4Addr { self.range.base_addr() }
	pub fn route(&self) -> Ipv4Route { self.range.into() }
	pub fn unique_addr(&mut self) -> Ipv4Addr {
		let addr = self.range.address_for(self.devices);
        self.devices += 1;
		addr
	}
	pub fn network_info(&self) -> NetworkInfo {
		NetworkInfo {
			connections: self.connections.iter().map(|(_, (id, _))|*id).collect(),
			ip_range: self.range.clone()
		}
	}

	pub fn runtime(&mut self) -> Result<&mut NetworkRuntime, NetworkError> {
		self.runtime.as_mut().ok_or(NetworkError::NoRuntime)
	}
	pub fn connect(&mut self, wire_idx: WireIdx, node_id: NodeIdx, routes: Vec<Ipv4Route>) -> Result<Plug, NetworkError> {
		let (router_plug, outgoing_plug) = netsim_embed::wire();
		self.connections.insert(wire_idx, (node_id, routes.clone()));
		self.runtime()?.router.add_connection(node_id.as_usize(), router_plug, routes); Ok(outgoing_plug)
	}
	pub fn disconnect(&mut self, idx: WireIdx) -> Result<(), NetworkError> {
		let (node_id, _) = self.connections[idx];
		self.connections.remove(idx);
		task::block_on(self.runtime()?.router.remove_connection(node_id.as_usize())); Ok(())
	}
}

#[derive(Debug, Clone)]
pub enum NodeType {
	Network,
	Machine,
}

#[derive(Serialize, Deserialize)]
pub enum NodeVariant {
	Network(InternetNetwork),
	Machine(InternetMachine),
}

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
#[derive(Serialize, Deserialize)]
pub struct InternetNode {
	pub variant: NodeVariant,
	/// Position of this Node in the Internet Simulation Field
	position: FieldPosition,
	/// Index of Node in Internet.map
	pub id: NodeIdx,
}

impl InternetNode {
	pub fn from_machine(machine: InternetMachine, position: FieldPosition, id: NodeIdx) -> Self {
		Self {
			variant: NodeVariant::Machine(machine),
			position, id,
		}
	}
	pub fn from_network(network: InternetNetwork, position: FieldPosition, id: NodeIdx) -> Self {
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
				(machine.latency(), machine.connection.map(|(_, addr)|addr), NodeType::Machine)
			},
		};
		NodeInfo {
			position: self.position.clone(),
			internal_latency,
			local_address,
			node_type,
		}
	}
	pub fn disconnect(&mut self, wire_idx: WireIdx) -> Result<(), InternetError> {
		match &mut self.variant {
			NodeVariant::Machine(machine) => machine.disconnect()?,
			NodeVariant::Network(network) => network.disconnect(wire_idx)?,
		}
		Ok(())
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

