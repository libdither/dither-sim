/// Defines all the generic components of a node interacting with an internet structure.
/// A Node should be able to work in any kind of network. simulated or not. This file provides the basic structures that any network implementation will use to interact with a Node.

use tokio::{net::TcpStream};
//use futures::{AsyncBufRead, AsyncWrite};

use crate::{NodeID, RouteCoord};

/// Address that allows a Node to connect to another Node over a network implementation. This might be an IP address, a multiaddr, or just a number.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Archive, Serialize, Deserialize, serde::Serialize, serde::Deserialize)]
#[archive_attr(derive(bytecheck::CheckBytes))]
#[repr(transparent)]
pub struct Address(pub Vec<u8>);

/// Represents a 2-way asyncronous stream of bytes and the address used to establish the connection.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Connection {
	/// Data object that allows an underlying protocol to route packets to another computer
	pub address: Address,
	/// Two-way Binary Stream object that connects losslessly to another remote peer
	#[derivative(Debug="ignore")]
	pub stream: TcpStream, // This will be a bytestream provided by the network implementation (i.e. libp2p)
}

/// Response Object sent wrapped in a NetAction when a connection is requested
#[derive(Debug)]
pub enum ConnectionResponse {
	/// Established Connection
	Established(Connection),
	/// Remote could not be located
	NotFound,
	/// Remote exists, but there was an error in establishing the connection. 
	Error(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo {
	pub node_id: NodeID,
	pub route_coord: Option<RouteCoord>,
	pub public_addr: Option<Address>,
	pub remotes: usize,
	pub active_remotes: usize,
}

/// Actions that can be sent to the Network Implementation (Most of these are temporary)
/// [External] represents the program that interacts with this instance of the Dither API
/// This represents the system-facing protocol used by the p2p network implementation in addition to externals
#[derive(Debug)]
pub enum NetAction {
	/// From Node
	/// [Dither -> External] Publish Route to "fake" DHT (will be replaced with real kademlia DHT or reverse hash lookup implementation )
	PublishRouteCoords(NodeID, RouteCoord),
	/// [Dither -> Wrapper] Query Route Coords from "fake" DHT
	QueryRouteCoord(NodeID),
	/// [External/ -> Dither] Response for QueryRouteCoord Action
	QueryRouteCoordResponse(NodeID, RouteCoord),

	/// [Dither -> External/Network] Establish a Connection via a multiaddress (interpreted by network impl)
	Connect(Address),
	/// [External/Network -> Dither] Repond to a connection request
	ConnectResponse(ConnectionResponse),
	/// [External/Network -> Dither] Notify incoming connection
	Incoming(Connection),

	/// [External/System -> Dither] Request info about Dither Node
	GetNodeInfo,
	/// [Dither -> External/System] Info about node
	NodeInfo(NodeInfo),	
}