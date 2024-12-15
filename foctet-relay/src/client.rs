use std::collections::{HashMap, HashSet};

use bytes::BytesMut;
use foctet::{core::{frame::Frame, node::{ConnectionId, NodeId}}, net::connection::{NetworkStream, FoctetStream}};
use tokio::sync::mpsc;
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use anyhow::Result;
use crate::message::{Packet, ServerMessage};

#[derive(Debug)]
struct ClientActor {
    /// The node ID of this client.
    node_id: NodeId,
    /// The network stream between this client and the server.
    stream: NetworkStream,
    /// The queue to receive packets from the server to this client.
    /// Other peers will send packets to this client through this queue.
    packet_queue_rx: mpsc::Receiver<Packet>,
    /// Server channel to send messages to the server.
    server_channel_tx: mpsc::Sender<ServerMessage>,
}

impl ClientActor {
    async fn run(mut self, cancel: CancellationToken) -> Result<()> {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Client actor loop cancelled, closing loop");
                    break;
                }
                Some(packet) = self.packet_queue_rx.recv() => {
                    self.send_packet(packet).await;
                }
                Ok(bytes) = self.stream.receive_bytes() => {
                    self.handle_framed_bytes(bytes).await;
                }
            }
        }
        Ok(())
    }
    async fn send_packet(&mut self, packet: Packet) {
        // Send the packet to the connected client
        if let Err(e) = self.stream.send_bytes(packet.data).await {
            tracing::warn!("Error sending packet to client: {:?}", e);
        }
    }
    async fn handle_framed_bytes(&mut self, bytes: BytesMut) {
        let dst_node_id = NodeId::zero();
        let packet = Packet::new(self.node_id.clone(), dst_node_id, bytes.freeze());
        if let Err(e) = self.server_channel_tx.send(ServerMessage::SendPacket(packet)).await {
            tracing::warn!("Error sending packet to server: {:?}", e);
        }
    }
}

#[derive(Debug)]
pub struct ClientConnection {
    /// The connection number of this client.
    connection_id: ConnectionId,
    /// The node ID of this client.
    node_id: NodeId,
    /// Client actor handle.
    handle: AbortOnDropHandle<()>,
    /// The queue to send packets from this client to the server.
    /// This client will send packets to the other peers through this queue.
    packet_queue_tx: mpsc::Sender<Packet>,
    /// Cancellation token to stop the actor loop.
    cancel: CancellationToken,
}

impl ClientConnection {
    pub async fn shutdown(self) {
        // Cancel the client connection
        self.cancel.cancel();
        if let Err(e) = self.handle.await {
            tracing::warn!(
                "Error closing actor loop for client connection {:?} {}: {e:?}",
                self.node_id, self.connection_id.as_str(),
            );
        };
    }
}

#[derive(Debug)]
pub struct Client {
    /// The node ID of this client.
    node_id: NodeId,
    /// The connection between this client and the server.
    conn: ClientConnection,
    /// The known peers of this client.
    known_peers: HashSet<NodeId>,
}

impl Client {
    fn new(node_id: NodeId, conn: ClientConnection) -> Self {
        Self {
            node_id,
            conn,
            known_peers: HashSet::new(),
        }
    }
    pub async fn shutdown(self) {
        // Cancel the client connection
        self.conn.shutdown().await;
    }
}

#[derive(Debug)]
pub struct ClientManager {
    /// The map of client connections.
    pub clients: HashMap<NodeId, Client>,
    /// The next connection ID.
    connection_id: ConnectionId,
}

impl ClientManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            connection_id: ConnectionId::new(0),
        }
    }
    pub fn next_connection_id(&mut self) -> ConnectionId {
        self.connection_id.increment();
        self.connection_id
    }
    pub async fn shutdown(&mut self) {
        self.clients.drain().for_each(|(_node_id, client)| {
            tokio::spawn(async move {
                client.shutdown().await;
            });
        });
    }
    pub async fn send_packet(&mut self, packet: Packet) {

    }
    pub async fn register(&mut self, node_id: NodeId, stream: NetworkStream) {
        // add a new client connection
        // if the client already exists, shutdown the existing connection
        if let Some(client) = self.clients.remove(&node_id) {
            tokio::spawn(async move {
                client.shutdown().await;
            });
        }
        let (packet_queue_tx, packet_queue_rx) = mpsc::channel(100);
        let (server_channel_tx, server_channel_rx) = mpsc::channel(100);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let connection_id = self.next_connection_id();
        let actor = ClientActor {
            node_id: node_id.clone(),
            stream,
            packet_queue_rx,
            server_channel_tx,
        };
        let handle = AbortOnDropHandle::new(tokio::spawn(async move {
            let _ = actor.run(cancel_clone).await;
        }));
        let conn = ClientConnection {
            connection_id,
            node_id: node_id.clone(),
            handle,
            packet_queue_tx,
            cancel,
        };
        let client = Client::new(node_id.clone(), conn);
        self.clients.insert(node_id, client);
    }
    pub async fn unregister(&mut self, node_id: NodeId) {

    }
}
