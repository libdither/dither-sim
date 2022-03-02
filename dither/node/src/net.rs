use bytecheck::CheckBytes;
use futures::{AsyncBufRead, AsyncWrite};
use rkyv::{AlignedVec, Archive, Deserialize, Infallible, Serialize, ser::serializers::{AlignedSerializer, AllocScratch, CompositeSerializer, FallbackScratch, HeapScratch, SharedSerializeMap}, validation::validators::DefaultValidator};
/// Defines all the generic components of a node interacting with an internet structure.
/// A Node should be able to work in any kind of network. simulated or not. This file provides the basic structures that any network implementation will use to interact with a Node.
//use futures::{AsyncBufRead, AsyncWrite};

use crate::{NodeID, RouteCoord};

pub trait Address: 
{}

/// Create Network implementation
pub trait Network: Clone + Send + Sync + std::fmt::Debug + 'static
{
	/// Represents potential Connection that can be established by Network implementation
	type Addr: Clone + PartialEq + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync
	+ for<'de> serde::Deserialize<'de>
	+ serde::Serialize
	+ for<'b> Serialize<CompositeSerializer<AlignedSerializer<&'b mut AlignedVec>, FallbackScratch<HeapScratch<256_usize>, AllocScratch>, SharedSerializeMap>>
	+ Archive<Archived = Self::ArchivedAddr>;

	type ArchivedAddr: Deserialize<Self::Addr, Infallible> + for<'v> CheckBytes<DefaultValidator<'v>> + Send + Sync;
	/// Bidirectional byte stream for sending and receiving NodePackets
	type Conn: AsyncBufRead + AsyncWrite + std::fmt::Debug + Send + Sync + Clone + Unpin;
}

/// Response Object sent wrapped in a NetAction when a connection is requested
#[derive(Debug)]
pub enum ConnectionResponse<Net: Network> {
	/// Established Connection
	Established(Net::Conn),
	/// Remote could not be located
	NotFound,
	/// Remote exists, but there was an error in establishing the connection. 
	Error(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo<Net: Network> {
	pub node_id: NodeID,
	pub route_coord: Option<RouteCoord>,
	pub public_addr: Option<Net::Addr>,
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
	Connect(Net::Addr),
	/// [Network -> Dither] Reponse to connection request
	ConnectResponse(Net::Addr, ConnectionResponse<Net>),
	/// [Network -> Dither] Notify incoming connection
	Incoming(Net::Addr, Net::Conn),

	/// [User -> Dither] Request info about Dither Node
	GetNodeInfo,
	/// [Dither -> User] Return Info about node
	NodeInfo(NodeInfo<Net>),	
}