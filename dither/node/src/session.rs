//! This session module manages the ongoing state of a connection to a remote node. It deals with encryption and packet parsing.
//! It has two "threads" that manage reading and writing, and both report back to the RemoteNode via RemoteActions

#![allow(unused)]

use std::marker::PhantomData;

use async_std::task::{self, JoinHandle};
use futures::{Sink, SinkExt, StreamExt, channel::mpsc::{self, Sender}, FutureExt};
use rkyv::{AlignedVec, Archived, Deserialize, Infallible};
use rkyv_codec::{RkyvWriter, archive_stream, VarintLength, RkyvCodecError};

use crate::{net::Network, packet::{NodePacket, ArchivedNodePacket}, remote::RemoteAction};

pub type SessionKey = u128;

#[derive(Debug)]
pub enum SessionAction<Net: Network> {
	NewConnection(Net::Conn),
	SendPacket(NodePacket<Net>),
	CloseSession,
}

#[derive(Error, Debug)]
pub enum SessionError {
	#[error("Tunnel Closed")]
	TunnelClosed,
	#[error("Failed to Send to Remote Thread")]
	RemoteSendError(#[from] mpsc::SendError),
	#[error("Packet Stream Error")]
	PacketCodecError(#[from] RkyvCodecError),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Session<Net: Network> {
	key: SessionKey,
	_data: PhantomData<Net>,
}
impl<Net: Network> Session<Net> {
	pub fn new() -> Self {
		Self { key: rand::random(), _data: Default::default() }
	}
	async fn handle_packet<'b>(&mut self, packet: &'b Archived<NodePacket<Net>>, remote_action: &mut Sender<RemoteAction<Net>>) -> Result<Option<NodePacket<Net>>, SessionError> {
		let packet = match packet {
			ArchivedNodePacket::Init { init_session_key, initiating_id, receiving_id } => {
				None
			},
			ArchivedNodePacket::Session { session_key, encrypted_packet } => {
				if *session_key == self.key {
					let packet = NodePacket::from_archive(encrypted_packet);
					remote_action.send(RemoteAction::ReceivePacket(packet)).await?;
				} else {
					log::error!("Received Badly Encrypted Packet")
				}
				None
			}
			_ => {
				let send_back: NodePacket<Net> = NodePacket::from_archive(packet);
				Some(NodePacket::BadPacket { packet: Box::new(send_back) })
			},
		};
		Ok(packet)
	}
	pub fn start(mut self, address: Net::Address, connection: Net::Conn, mut remote_action: Sender<RemoteAction<Net>>) -> (JoinHandle<Session<Net>>, Sender<SessionAction<Net>>) {
		let (action_sender, mut action_receiver) = mpsc::channel::<SessionAction<Net>>(20);
		let join_handle = task::spawn(async move {
			// Writing Thread, Listens to action_receiver and occasionally writes to writer
			
			let mut stream_buffer = AlignedVec::with_capacity(1024);
			let packet_reader = connection.clone();
			let packet_sink = RkyvWriter::<<Net as Network>::Conn, VarintLength>::new(connection);
			futures::pin_mut!(packet_sink);
			futures::pin_mut!(packet_reader);
			
			loop {
				let stream_future = archive_stream::<Net::Conn, NodePacket<Net>, VarintLength>(&mut packet_reader, &mut stream_buffer).fuse();
				futures::pin_mut!(stream_future);

				futures::select! {
					// Receive Actions, Write Packets
					action = action_receiver.next() => {
						if let Some(action) = action {
							match action {
								SessionAction::SendPacket(packet) => {
									if let Err(_) = packet_sink.send(&packet).await { log::error!("Failed to send packet") }
								}
								_ => { log::error!("Session Received wrong action: {:?}", action); }
							}
						} else {
							log::error!("Session with peer {:?} Closed", address);
						}
					},
					packet = stream_future => {
						if let Ok(packet) = packet {
							if let Ok(Some(packet)) = self.handle_packet(packet, &mut remote_action).await {
								log::info!("Sending packet: {:?}", packet);
								packet_sink.send(&packet).await;
							}
						} else { log::error!("Packet Stream with {:?} closed", address); }
					},
				}
			}
			

			// Return self in Join Handle
			self
		});
			
		// Returns Join Handle and method of
		(join_handle, action_sender)
	}
	
}