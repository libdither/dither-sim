use std::time::Duration;
use std::mem;

use async_std::{self, task::{self, JoinHandle}};
use futures::{SinkExt, StreamExt, channel::mpsc, select};
use netsim_embed::Plug;

use futures_delay_queue::delay_queue;
enum WireAction {
	SetDelay(Duration),
	SwapPlugA(Plug),
	SwapPlugB(Plug),

	Disconnect, // Will wait for buffered packets to send
	ForceDisconnect, // Disconnects wire immediately, may drop packets
}
enum WireReturn {
	SwappedPlugA(Plug),
	SwappedPlugB(Plug),
}

pub struct Wire {
    pub delay: Duration,
}

impl Wire {
	pub fn connect(mut self, plug_a: Plug, plug_b: Plug) -> WireHandle {
		let (action_sender, mut action_receiver) = mpsc::channel(5);
		let (mut return_sender, return_receiver) = mpsc::channel(1);

		let (mut a_tx, mut a_rx) = plug_a.split();
		let (mut b_tx, mut b_rx) = plug_b.split();

		let join_handle = task::spawn(async move {
			let (delay_queue_a_to_b, packet_to_b) = delay_queue::<Vec<u8>>();
			let (delay_queue_b_to_a, packet_to_a) = delay_queue::<Vec<u8>>();

			let mut disconnecting = false;
			loop {
				select! {
					action = action_receiver.next() => {
						if let Some(action) = action {
							match action {
								WireAction::SetDelay(delay) => self.delay = delay,
								WireAction::SwapPlugA(new_plug) => {
									let (mut tx, mut rx) = new_plug.split();
									mem::swap(&mut tx, &mut a_tx); mem::swap(&mut rx, &mut a_rx);
									let old_plug = Plug::join(tx, rx);
									return_sender.send(WireReturn::SwappedPlugA(old_plug)).await.unwrap();
								},
								WireAction::SwapPlugB(new_plug) => {
									let (mut tx, mut rx) = new_plug.split();
									mem::swap(&mut tx, &mut b_tx); mem::swap(&mut rx, &mut b_rx);
									let old_plug = Plug::join(tx, rx);
									return_sender.send(WireReturn::SwappedPlugB(old_plug)).await.unwrap();
								},
								WireAction::Disconnect => { disconnecting = true; break },
								WireAction::ForceDisconnect => break,
							}
						}
					}
					a_incoming_data = a_rx.next() => {
						if let Some(data) = a_incoming_data {
							delay_queue_a_to_b.insert(data, self.delay);
						}
					}
					b_incoming_data = b_rx.next() => {
						if let Some(data) = b_incoming_data {
							delay_queue_b_to_a.insert(data, self.delay);
						}
					}
					a_outgoing_data = packet_to_a.receive() => {
						if let Some(data) = a_outgoing_data {
							a_tx.send(data).await.unwrap();
						}
					}
					b_outgoing_data = packet_to_b.receive() => {
						if let Some(data) = b_outgoing_data {
							b_tx.send(data).await.unwrap();
						}
					}
				}
			}
			
			// TODO: This one_is_done, two_is_done thing feels really janky, there has got to be a better way to do this
			let mut one_is_done = false;
			let mut two_is_done = false;
			if disconnecting {
				loop {
					select! {
						outgoing_a = packet_to_a.receive() => {
							if let Some(data) = outgoing_a {
								a_tx.send(data).await.unwrap();
							} else { if two_is_done { break } else { one_is_done = true; } }
						}
						outgoing_b = packet_to_b.receive() => {
							if let Some(data) = outgoing_b {
								b_tx.send(data).await.unwrap();
							} else { if one_is_done { break } else { two_is_done = true; } }
						}
					}
				}
			}
			
			(self, Plug::join(a_tx, a_rx), Plug::join(b_tx, b_rx))
		});
		WireHandle { join_handle, action_sender, return_receiver, did_error: None }
	}
}

pub struct WireHandle {
	join_handle: JoinHandle<(Wire, Plug, Plug)>,
	action_sender: mpsc::Sender<WireAction>,
	return_receiver: mpsc::Receiver<WireReturn>,
	did_error: Option<mpsc::SendError>,
}
impl WireHandle {
	async fn action(&mut self, action: WireAction) {
		match self.action_sender.send(action).await {
			Err(err) => self.did_error = Some(err), _ => {},
		}
	}
	pub async fn swap_a_plug(&mut self, plug_a: Plug) -> Option<Plug> {
		self.action(WireAction::SwapPlugA(plug_a)).await;
		if let Some(WireReturn::SwappedPlugA(plug)) = self.return_receiver.next().await {
			Some(plug)
		} else { None }
	}
	pub async fn swap_b_plug(&mut self, plug_b: Plug) -> Option<Plug> {
		self.action(WireAction::SwapPlugB(plug_b)).await;
		if let Some(WireReturn::SwappedPlugB(plug)) = self.return_receiver.next().await {
			Some(plug)
		} else { None }
	}
	pub async fn set_delay(&mut self, delay: Duration) {
		self.action(WireAction::SetDelay(delay)).await;
	}
	pub async fn disconnect(mut self) -> (Wire, Plug, Plug) {
		self.action(WireAction::Disconnect).await;
		self.join_handle.await
	}
	pub async fn force_disconnect(mut self) -> (Wire, Plug, Plug) {
		self.action(WireAction::ForceDisconnect).await;
		self.join_handle.await
	}
}