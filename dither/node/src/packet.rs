

use std::fmt;

use bytecheck::CheckBytes;
use futures::SinkExt;
use rkyv::{AlignedVec, Archive, Archived, Deserialize, Infallible, Serialize, with::Inline};
use rkyv_codec::{RkyvCodecError, RkyvWriter, VarintLength, archive_stream};

use crate::{net::{Connection, Network}};
use super::{NodeID, RouteCoord};

/// Acknowledging node packet
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct AckNodePacket<'a, Net: Network> {
	#[with(Inline)]
	pub packet: &'a NodePacket<Net>, // The packet being sent
	pub packet_id: u16, // This packet's packet id
	pub should_ack: bool, // Should the node that receives this packet immediately send back an acknowledgement
	pub acknowledging: Option<u16>, // Packet that this packet is acknowledging
}

/// Packets that are sent between nodes in this protocol.
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[archive(bound(serialize = "__S: rkyv::ser::ScratchSpace + rkyv::ser::Serializer"))]
#[archive_attr(derive(CheckBytes, Debug), check_bytes(bound = "__C: rkyv::validation::ArchiveContext, <__C as rkyv::Fallible>::Error: bytecheck::Error"))]
pub enum NodePacket<Net: Network> {
	/// Bootstrap off of a node
	Bootstrap {
		requester: NodeID,
	},

	/// Tell another node my info
	Info {
		route_coord: RouteCoord,
		active_peers: usize,
	},

	/// Request a certain number of another node's peers that are closest to this node to make themselves known
	RequestPeers {
		nearby: Vec<(RouteCoord, usize)>
	},

	/// Notify peer near `requesting` that the `requesting` node is looking for a peer.
	WantPeer {
		requesting: NodeID,
		addr: Net::Address
	},

	WantPeerResp {
		prompting_node: NodeID,
	},

	Notify {
		active: bool,
	},

	/// `Ack` packet
	/// used to respond to acknowledge packets if there is no other suitable acknowledgement packet.
	Ack,

	/// Raw Data Packet
	Data(Vec<u8>),

	/// Traversing packet
	Traversal {
		/// Place to Route Packet to
		destination: RouteCoord,
		/// Packet to traverse to destination node
		#[omit_bounds] #[archive_attr(omit_bounds)] session_packet: Box<NodePacket<Net>>, // Must be type Init or Session packet
	},

	/// Packet representing an origin location
	Return {
		#[omit_bounds] #[archive_attr(omit_bounds)] packet: Box<NodePacket<Net>>,
		origin: RouteCoord,
	},
}
impl<Net: Network> NodePacket<Net> 
where <Net::Address as Archive>::Archived: Deserialize<Net::Address, Infallible>
{
	pub fn from_archive(archive: &Archived<NodePacket<Net>>) -> Self
	{
		Deserialize::<NodePacket<Net>, Infallible>::deserialize(archive, &mut Infallible).unwrap()
	}
	pub fn create_codec(connection: Connection<Net>, known_node_id: &NodeID) -> Option<(Net::Address, PacketRead<Net>, PacketWrite<Net>)> {
		let Connection { node_id, addr, read, write } = connection;
		if node_id == *known_node_id {
			Some((addr, PacketRead::new(read), PacketWrite::new(write)))
		} else { None }
	}
}

pub struct PacketRead<Net: Network> {
	reader: Net::Read,
	stream_buffer: AlignedVec,
}
impl<Net: Network> std::fmt::Debug for PacketRead<Net> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("PacketRead").finish() }
}
impl<'b, Net: Network> PacketRead<Net> {
	pub fn new(reader: Net::Read) -> Self { Self { reader, stream_buffer: AlignedVec::with_capacity(1024) } }
	pub async fn read_packet(&'b mut self) -> Result<&'b Archived<AckNodePacket<'b, Net>>, RkyvCodecError> {
		let packet = archive_stream::<Net::Read, AckNodePacket<Net>, VarintLength>(&mut self.reader, &mut self.stream_buffer).await?;
		Ok(packet)
	}
}
pub struct PacketWrite<Net: Network> {
	writer: RkyvWriter<Net::Write, VarintLength>,
}
impl<Net: Network> std::fmt::Debug for PacketWrite<Net> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("PacketWrite").finish() }
}
impl<Net: Network> PacketWrite<Net> {
	pub fn new(writer: Net::Write) -> Self { Self { writer: RkyvWriter::new(writer) } }
	pub async fn write_packet<'a>(&mut self, packet: &AckNodePacket<'a, Net>) -> Result<(), RkyvCodecError> {
		Ok(self.writer.send(packet).await?)
	}
}