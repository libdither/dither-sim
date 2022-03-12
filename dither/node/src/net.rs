//! Defines all the generic components of a node interacting with an internet structure.
//! A Node should be able to work in any kind of network. simulated or not. This file provides the basic structures that any network implementation will use to interact with a Node, in addition to any structures a User will use to interact with the network implementation and by extension, the Node.

use std::fmt;

use bytecheck::CheckBytes;
use futures::{AsyncRead, AsyncWrite};
use rkyv::{AlignedVec, Archive, Deserialize, Infallible, Serialize, ser::serializers::{AlignedSerializer, AllocScratch, CompositeSerializer, FallbackScratch, HeapScratch, SharedSerializeMap}, validation::validators::DefaultValidator};


use crate::{NodeAction, NodeID, RouteCoord};

/// Create Network implementation
pub trait Network: Clone + Send + Sync + std::fmt::Debug + 'static
{
	/// Represents potential Connection that can be established by Network implementation
	type Address: Clone + PartialEq + Eq + std::hash::Hash + fmt::Debug + Send + Sync + fmt::Display
	+ for<'de> serde::Deserialize<'de>
	+ serde::Serialize
	+ for<'b> Serialize<CompositeSerializer<AlignedSerializer<&'b mut AlignedVec>, FallbackScratch<HeapScratch<256_usize>, AllocScratch>, SharedSerializeMap>>
	+ Archive<Archived = Self::ArchivedAddress>;

	type ArchivedAddress: fmt::Debug + Deserialize<Self::Address, Infallible> + for<'v> CheckBytes<DefaultValidator<'v>> + Send + Sync;
	/// Bidirectional byte stream for sending and receiving NodePackets
	type Read: AsyncRead + Send + Sync + Clone + Unpin;
	type Write: AsyncWrite + Send + Sync + Clone + Unpin;
}

pub struct Connection<Net: Network> {
	pub addr: Net::Address,
	pub read: Net::Read,
	pub write: Net::Write
}
impl<Net: Network> fmt::Debug for Connection<Net> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("Connection").field("addr", &self.addr).finish() }
}

/// Response Object sent wrapped in a NetAction when a connection is requested
#[derive(Debug)]
pub enum ConnectionResponse<Net: Network> {
	/// Established Connection
	Established(Connection<Net>),
	/// Remote could not be located
	NotFound(Net::Address),
	/// Remote exists, but there was an error in establishing the connection. 
	Error(Net::Address, String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo<Net: Network> {
	pub node_id: NodeID,
	pub route_coord: RouteCoord,
	pub public_addr: Option<Net::Address>,
	pub remotes: usize,
	pub active_remotes: usize,
}

/// Actions that the User can send to manage the network
#[derive(Debug)]
pub enum UserAction<Net: Network>{
	NodeAction(Box<NodeAction<Net>>),
	GetNodeInfo,
}
/// Events received by the User about the network state
#[derive(Debug)]
pub enum UserEvent<Net: Network> {
	/// [Dither -> User] Return Info about node
	NodeInfo(NodeInfo<Net>),	
}

/// Actions sent from Dither to the Network implementation
#[derive(Debug)]
pub enum NetAction<Net: Network> {
	/// Connect to some remote
	Connect(Net::Address),

	/// Returned User Event
	UserEvent(UserEvent<Net>),
}

/// Events produced by the network for the Dither implementation

/// Actions that can be sent to the Network Implementation (Most of these are temporary)
/// [External] represents the program that interacts with this instance of the Dither API
/// This represents the system-facing protocol used by the p2p network implementation in addition to externals
#[derive(Debug)]
pub enum NetEvent<Net: Network> {
	/// Connection response
	ConnectResponse(ConnectionResponse<Net>),
	/// Unprompted connection
	Incoming(Connection<Net>),
	/// Notify incoming UserAction for Node
	UserAction(UserAction<Net>),
}