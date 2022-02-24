use futures::{AsyncBufRead, AsyncWrite};
/// Defines all the generic components of a node interacting with an internet structure.
/// A Node should be able to work in any kind of network. simulated or not. This file provides the basic structures that any network implementation will use to interact with a Node.
//use futures::{AsyncBufRead, AsyncWrite};

use crate::{NodeID, RouteCoord};

/// Create Network implementation
pub trait Network {
	/// Represents potential Connection that can be established by Network implementation
	type Address: Clone + PartialEq + Eq + std::hash::Hash + std::fmt::Debug;
	/// Bidirectional byte stream for sending and receiving NodePackets
	type Connection: AsyncBufRead + AsyncWrite + std::fmt::Debug;
}

/// Response Object sent wrapped in a NetAction when a connection is requested
#[derive(Debug)]
pub enum ConnectionResponse<Net: Network> {
	/// Established Connection
	Established(Net::Connection),
	/// Remote could not be located
	NotFound,
	/// Remote exists, but there was an error in establishing the connection. 
	Error(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo<Net: Network> {
	pub node_id: NodeID,
	pub route_coord: Option<RouteCoord>,
	#[serde(bound(serialize = "Net::Address: serde::Serialize", deserialize = "Net::Address: serde::Deserialize<'de>"))]
	pub public_addr: Option<Net::Address>,
	pub remotes: usize,
	pub active_remotes: usize,
}

/// Actions that can be sent to the Network Implementation (Most of these are temporary)
/// [External] represents the program that interacts with this instance of the Dither API
/// This represents the system-facing protocol used by the p2p network implementation in addition to externals
#[derive(Debug)]
pub enum NetAction<Net: Network> {
	/// From Node
	/// [Dither -> Temp Network] Publish Route to "fake" DHT (will be replaced with real kademlia DHT or reverse hash lookup implementation )
	PublishRouteCoords(NodeID, RouteCoord),
	/// [Dither -> Temp Network] Query Route Coords from "fake" DHT
	QueryRouteCoord(NodeID),
	/// [Temp Network -> Dither] Response for QueryRouteCoord Action
	QueryRouteCoordResponse(NodeID, RouteCoord),

	/// [Dither -> Network] Establish a Connection via a multiaddress (interpreted by network impl)
	Connect(Net::Address),
	/// [Network -> Dither] Reponse to connection request
	ConnectResponse(Net::Address, ConnectionResponse<Net>),
	/// [Network -> Dither] Notify incoming connection
	Incoming(Net::Address, Net::Connection),

	/// [User -> Dither] Request info about Dither Node
	GetNodeInfo,
	/// [Dither -> User] Return Info about node
	NodeInfo(NodeInfo<Net>),	
}