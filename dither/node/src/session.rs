//! This session module manages the ongoing state of a connection to a remote node. It deals with encryption and packet parsing.
//! It has two "threads" that manage reading and writing, and both report back to the RemoteNode via RemoteActions

#![allow(unused)]

use std::marker::PhantomData;

use async_std::task::{self, JoinHandle};
use tokio_util::codec::{Framed};
use futures::{Sink, SinkExt, StreamExt, channel::mpsc::{self, Sender}};

use crate::{net::Network, packet::NodePacket, remote::RemoteAction};
use crate::packet::PacketCodec;

pub type SessionKey = u128;

#[derive(Debug)]
pub enum SessionAction<Net: Network> {
	NewConnection(Net::Connection),
	SendPacket(NodePacket<Net>),
	CloseSession,
}

#[derive(Error, Debug)]
pub enum SessionError {
	#[error("Tunnel Closed")]
	TunnelClosed,
	#[error(transparent)]
	IoError(#[from] std::io::Error),
	#[error("Failed to Send to Remote Thread")]
	RemoteSendError(#[from] mpsc::SendError),
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
	async fn handle_packet(&mut self, packet: NodePacket<Net>, remote_action: &Sender<RemoteAction<Net>>) -> Result<Option<NodePacket>, SessionError> {
		let packet = match packet {
			NodePacket::Init { init_session_key, initiating_id, receiving_id } => {
				None
			},
			NodePacket::Session { session_key, encrypted_packet } => {
				if session_key == self.key {
					remote_action.send(RemoteAction::ReceivePacket(*encrypted_packet)).await?;
				} else {
					log::error!("Received Badly Encrypted Packet")
				}
				None
			}
			_ => Some(NodePacket::BadPacket { packet: Box::new(packet) }),
		};
		Ok(packet)
	}
	pub fn start(mut self, connection: Net::Connection, remote_action: Sender<RemoteAction<Net>>) -> (JoinHandle<Session<Net>>, Sender<SessionAction<Net>>) {
		let (action_sender, mut action_receiver) = mpsc::channel::<SessionAction>(20);
		let join_handle = task::spawn(async move {
			// Writing Thread, Listens to action_receiver and occasionally writes to writer
			
			// Frame Connection Stream with Packet Codec
			let mut packet_stream = PacketCodec::new(connection.stream);
			loop {
				let error: Result<(), SessionError> = try {
					futures::select!{
						// Receive Actions, Write Packets
						action = action_receiver.recv() => {
							if let Some(action) = action {
								match action {
									SessionAction::SendPacket(packet) => {
										log::info!("Received Packet: {:?}", packet);
									}
									_ => { log::error!("Session Received wrong action: {:?}", action) }
								}
							} else {
								log::error!("Session with {:?} Closed", connection.address);
								break;
							}
							
						},
						packet = packet_stream.next() => {
							let packet = packet.ok_or(SessionError::TunnelClosed)??;
							if let Some(packet) = self.handle_packet(packet, &remote_action).await? {
								packet_stream.feed(packet).await?;
							}
						},
					};
				};
				// If error, notify Remote thread
				if let Err(error) = error { remote_action.send(RemoteAction::SessionError(Box::new(error))).await.unwrap() };
			}
			

			// Return self in Join Handle
			self
		});
			
		// Returns Join Handle and method of
		(join_handle, action_sender)
	}
	
}