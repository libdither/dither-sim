//! This session module manages the ongoing state of a connection to a remote node. It deals with encryption and packet parsing.
//! It has two "threads" that manage reading and writing, and both report back to the RemoteNode via RemoteActions

use tokio::{io::BufReader, sync::mpsc::{self, Sender, error::SendError}, task::{JoinError, JoinHandle}};

use crate::{NodeAction, net::{Address, Connection}, packet::NodePacket, remote::RemoteAction};

pub type SessionKey = u128;

#[derive(Debug)]
pub enum SessionAction {
	NewConnection(Connection),
	SendPacket(NodePacket),
	CloseSession,
}

#[derive(Error, Debug)]
pub enum SessionError {
	#[error("Tunnel Closed")]
	TunnelClosed,

}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Session {
	key: SessionKey,
}
impl Session {
	pub fn new() -> Self {
		Self { key: rand::random() }
	}
	pub fn start(self, connection: Connection, remote_action: Sender<RemoteAction>) -> (JoinHandle<Session>, Sender<SessionAction>) {
		let (action_sender, mut action_receiver) = mpsc::channel::<SessionAction>(20);
		let join_handle = tokio::spawn(async move {
			// Writing Thread, Listens to action_receiver and occasionally writes to writer
			// Split Reader / Writer
			let (reader, writer) = tokio::io::split(connection.stream);
			let reader = BufReader::new(reader);
			
			
			loop {
				tokio::select!{
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
					// Receive Packets, Write Actions
				}
			}

			// Return self in Join Handle
			self
		});


		
		// Returns Join Handle and method of
		(join_handle, action_sender)
	}
}