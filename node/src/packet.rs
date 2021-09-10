use crate::{net, session::SessionKey};

use super::{NodeID, RouteCoord};

/// Packets that are sent between nodes in this protocol.
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[archive(bound(serialize = "__S: rkyv::ser::ScratchSpace + rkyv::ser::Serializer"))]
#[archive_attr(derive(bytecheck::CheckBytes))]
pub enum NodePacket {
	/// Initiating Packet with unknown node
	InitUnknown { initiating_id: NodeID },
	/// Response to InitUnknown packet, Init packet might be sent after this
	InitAckUnknown { acknowledging_id: NodeID },
	/// Initial Packet, establishes encryption as well as some other things
	Init {
		initiating_id: NodeID,
		init_session_key: SessionKey,
		receiving_id: NodeID, // In future, Init packet will be asymmetrically encrypted with remote public key
	},

	/// Response to the Initial Packet, establishes encrypted tunnel.
	InitAck {
		ack_session_key: SessionKey, // Session key sent by Init, acknowledged
		acknowledging_id: NodeID,    // Previously receiving_id in Init packet
		receiving_id: NodeID,        // Previously initiating_id in Init packet
	},
	/// Sent back if received non-encrypted, non-init packet
	BadPacket {
		#[omit_bounds] packet: Box<NodePacket>,
	},

	/// All Packets that are not Init-type should be wrapped in session encryption
	Session {
		session_key: SessionKey,
		#[omit_bounds] encrypted_packet: Box<NodePacket>,
	},
	Traversal {
		/// Place to Route Packet to
		destination: RouteCoord,
		/// Packet to traverse to destination node
		#[omit_bounds] session_packet: Box<NodePacket>, // Must be type Init-type, or Session
		/// Signed & Assymetrically encrypted return location
		origin: Option<RouteCoord>,
	},

	/// ### Connection System
	/// Sent immediately after establishing encrypted session, allows other node to get a rough idea about the node's latency
	/// Contains list of packets for remote to respond to
	ConnectionInit {
		ping_id: u128,
		#[omit_bounds] initial_packets: Vec<NodePacket>,
	},

	/// Exchange Info with another node
	ExchangeInfo {
		/// Tell another node my Route Coordinate if I have it
		calculated_route_coord: Option<RouteCoord>,
		/// Number of direct connections I have
		useful_connections: usize,
		/// ping (latency) to remote node
		average_latency: u64,
		latency_accuracy: u32,
		response: bool,
	},

	/// Notify another node of peership
	/// * `usize`: Rank of remote in peer list
	/// * `RouteCoord`: My Route Coordinate
	/// * `usize`: Number of peers I have
	/// * `u64`: Latency to remote node
	PeerNotify(usize, RouteCoord, usize, u64),
	/// Propose routing coordinates if nobody has any nodes
	ProposeRouteCoords(RouteCoord, RouteCoord), // First route coord = other node, second route coord = myself
	/// Proposed route coords (original coordinates, orientation, bool), bool = true if acceptable
	ProposeRouteCoordsResponse(RouteCoord, RouteCoord, bool),

	/// ### Self-Organization System
	/// Request a certain number of another node's peers that are closest to this node to make themselves known
	/// * `usize`: Number of peers requested
	/// * `Option<RouteCoord>`: Route Coordinates of the other node if it has one
	RequestPings(usize, Option<RouteCoord>),

	/// Tell a peer that this node wants a ping (implying a potential direct connection)
	WantPing(NodeID, net::Address),
	/// Sent when node accepts a WantPing Request
	/// * `NodeID`: NodeID of Node who send the request in response to a RequestPings
	/// * `u64`: Distance to that nodeTraversedPacket
	AcceptWantPing(NodeID, u64),

	/* /// Request a session that is routed through node to another RouteCoordinate
	RoutedSessionRequest(RouteCoord),
	RoutedSessionAccept(), */
	/// Raw Data Packet
	Data(Vec<u8>),
}


use tokio_util::codec::{Encoder, Decoder};
use rkyv::{Archive, Deserialize, Infallible, Serialize, ser::{Serializer, serializers::{AllocScratch, CompositeSerializer, FallbackScratch, HeapScratch, SharedSerializeMap, WriteSerializer}}};
use bytes::{Buf, BufMut, BytesMut, buf::Writer};

const MAX_PACKET_LENGTH: usize = 1024 * 1024 * 16;

#[derive(Default)]
pub struct PacketCodec {}
pub type CodecSerializer<const N: usize> = CompositeSerializer<WriteSerializer<Writer<BytesMut>>, FallbackScratch<HeapScratch<N>, AllocScratch>, SharedSerializeMap>;

impl Encoder<NodePacket> for PacketCodec {
	type Error = std::io::Error;

	fn encode(&mut self, item: NodePacket, dst: &mut BytesMut) -> Result<(), Self::Error> {
		// Serialize Object
		
		let write_serializer = WriteSerializer::new(dst.writer()); // Create root serializer
		// Create compound serializer
		let scratch = FallbackScratch::<HeapScratch<4096>, AllocScratch>::default();
		let mut serializer = CompositeSerializer::new(write_serializer, scratch, SharedSerializeMap::default());
		
		serializer.pad(4).unwrap(); // Reserve space for length
		let item_len = serializer.serialize_value(&item).unwrap(); // Serialize object
		drop(serializer); // Drop serializer, dropping internal writer, which frees &mut BytesMut
		
		
		// Don't send a string if it is longer than the other end will accept
		if item_len > MAX_PACKET_LENGTH {
			Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Frame of length {} is too large.", item_len)
			))?;
		}

		// Overwrite Length Bytes
		(&mut dst[0..4]).put_u32_le(item_len as u32);
		Ok(())
	}
}

impl Decoder for PacketCodec {
	type Item = NodePacket;
	type Error = std::io::Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {

		// Check if large enough to read u32.
		if src.len() < 4 {
			return Ok(None);
		}

		// Copy length marker from buffer, interpret as u32 and cast to usize
		let length = src.copy_to_bytes(4).get_u32_le() as usize;

		// Check that the length is not too large to avoid a denial of
		// service attack where the server runs out of memory.
		if length > MAX_PACKET_LENGTH {
			return Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Frame of length {} is too large.", length)
			));
		}
		
		// Check if full string has arrived yet
		if src.len() < (4 + length) {
			// Reserve space (helps with efficiency, not required)
			src.reserve(4 + length - src.len());

			// We inform the Framed that we need more bytes to form the next
			// frame.
			return Ok(None);
		}
		let bytes = &src[4..4 + length];
		// interpret data from buffer as Archive, using validation feature to avoid unsafe code
		//let archived = rkyv::check_archived_root::<NodePacket>().unwrap(); // This doesn't work with deserialize for some reason
		let archived = unsafe { rkyv::archived_root::<NodePacket>(bytes) };
		// deserialize archive into an actual value
		let deserialized: NodePacket = archived.deserialize(&mut Infallible).unwrap();
		Ok(Some(deserialized))
	}
}