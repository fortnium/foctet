use std::collections::{HashMap, HashSet};

use bytes::BytesMut;
use foctet::{core::{error::StreamError, node::{ConnectionId, NodeId, NodePair}}, net::connection::{FoctetRecvStream, FoctetSendStream, FoctetStream, NetworkStream, RecvStream, SendStream}};
use tokio::sync::mpsc;
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use anyhow::Result;
use crate::message::{Packet, ServerMessage};

#[derive(Debug)]
struct ClientActor {
    /// The source node ID.
    src_node_id: NodeId,
    /// The destination node ID.
    dst_node_id: NodeId,
    /// The send side of the network stream between this client and the server.
    send_stream: SendStream,
    /// The receive side of the network stream between this client and the server.
    recv_stream: RecvStream,
    /// The queue to receive packets from the server to this client.
    /// Other peers will send packets to this client through this queue.
    packet_queue_rx: mpsc::Receiver<Packet>,
    /// Server channel to send messages to the server.
    server_channel_tx: mpsc::Sender<ServerMessage>,
}

impl ClientActor {
    async fn run(mut self, cancel: CancellationToken) -> Result<()> {
        if self.dst_node_id.is_zero() {
            tracing::info!("Starting relay control actor loop for {:?}", self.src_node_id);
        } else {
            tracing::info!("Starting relay client actor loop for {:?} -> {:?}", self.src_node_id, self.dst_node_id);
        }
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Client actor loop cancelled, closing loop");
                    break;
                }
                Some(packet) = self.packet_queue_rx.recv() => {
                    self.send_packet(packet).await;
                }
                /* Ok(bytes) = self.recv_stream.receive_bytes() => {
                    self.handle_framed_bytes(bytes).await;
                } */
                result = self.recv_stream.receive_bytes() => match result {
                    Ok(bytes) => {
                        self.handle_framed_bytes(bytes).await;
                    }
                    Err(e) => {
                        if let Some(e) = e.downcast_ref::<StreamError>() {
                            match e {
                                StreamError::Closed => {
                                    self.shutdown().await;
                                    tracing::info!("Client connection closed");
                                    break;
                                }
                                _ => {
                                    tracing::warn!("Error receiving bytes from client: {:?}", e);
                                }
                            }
                        } else {
                            tracing::warn!("Error receiving bytes from client: {:?}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    async fn send_packet(&mut self, packet: Packet) {
        // Send the packet to the connected client
        if let Err(e) = self.send_stream.send_bytes(packet.data).await {
            tracing::warn!("Error sending packet to client: {:?}", e);
        }
    }
    async fn handle_framed_bytes(&mut self, bytes: BytesMut) {
        let packet = Packet::new(self.src_node_id.clone(), self.dst_node_id.clone(), bytes.freeze());
        if let Err(e) = self.server_channel_tx.send(ServerMessage::SendPacket(packet)).await {
            tracing::warn!("Error sending packet to server: {:?}", e);
        }
    }
    async fn shutdown(&mut self) {
        // Cancel the client actor
        self.server_channel_tx.send(ServerMessage::RemoveClient(NodePair::new(self.src_node_id.clone(), self.dst_node_id.clone()))).await.unwrap();
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
    //node_id: NodeId,
    /// The connection between this client and the server.
    conn: ClientConnection,
    /// The known peers of this client.
    known_peers: HashSet<NodeId>,
}

impl Client {
    fn new(conn: ClientConnection) -> Self {
        Self {
            //node_id,
            conn,
            known_peers: HashSet::new(),
        }
    }
    pub fn add_known_peer(&mut self, node_id: NodeId) {
        self.known_peers.insert(node_id);
    }
    pub async fn shutdown(self) {
        // Cancel the client connection
        self.conn.shutdown().await;
    }
}

#[derive(Debug)]
pub struct ClientManager {
    /// The map of client connections.
    pub connections: HashMap<NodePair, Client>,
    /// The server channel to send messages to the server.
    pub server_channel_tx: mpsc::Sender<ServerMessage>,
    /// The next connection ID.
    connection_id: ConnectionId,
}

impl ClientManager {
    pub fn new(server_channel_tx: mpsc::Sender<ServerMessage>) -> Self {
        Self {
            connections: HashMap::new(),
            server_channel_tx,
            connection_id: ConnectionId::new(0),
        }
    }
    pub fn next_connection_id(&mut self) -> ConnectionId {
        self.connection_id.increment();
        self.connection_id
    }
    pub async fn shutdown(&mut self) {
        self.connections.drain().for_each(|(_node_pair, client)| {
            tokio::spawn(async move {
                client.shutdown().await;
            });
        });
    }
    pub async fn relay_packet(&mut self, packet: Packet) {
        let node_pair = NodePair::new(packet.dst.clone(), packet.src.clone());
        match self.connections.get_mut(&node_pair) {
            Some(client) => {
                client.add_known_peer(packet.src.clone());
                match client.conn.packet_queue_tx.send(packet).await {
                    Ok(_) => {
                        
                    },
                    Err(e) => {
                        tracing::warn!("Error sending packet to client: {:?}", e);
                    }
                }
            },
            None => {
                // Not found. Find the server connection and send the packet to the server
                let node_pair = NodePair::new(packet.dst.clone(), NodeId::zero());
                if let Some(client) = self.connections.get_mut(&node_pair) {
                    client.add_known_peer(packet.src.clone());
                    match client.conn.packet_queue_tx.send(packet).await {
                        Ok(_) => {
                            
                        },
                        Err(e) => {
                            tracing::warn!("Error sending packet to client: {:?}", e);
                        }
                    }
                } else {
                    tracing::warn!("Server connection not found for packet {:?}", packet);
                }
            }
        }
    }
    pub async fn register(&mut self, pair: NodePair, stream: NetworkStream, packet_queue_capacity: usize) {
        // add a new client connection
        // if the client already exists, shutdown the existing connection
        if let Some(client) = self.connections.remove(&pair) {
            tokio::spawn(async move {
                client.shutdown().await;
            });
        }
        let (packet_queue_tx, packet_queue_rx) = mpsc::channel(packet_queue_capacity);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let connection_id = self.next_connection_id();
        let (send_stream, recv_stream) = stream.split();
        let actor = ClientActor {
            src_node_id: pair.src.clone(),
            dst_node_id: pair.dst.clone(),
            send_stream,
            recv_stream,
            packet_queue_rx,
            server_channel_tx: self.server_channel_tx.clone(),
        };
        let handle = AbortOnDropHandle::new(tokio::spawn(async move {
            let _ = actor.run(cancel_clone).await;
        }));
        let conn = ClientConnection {
            connection_id,
            node_id: pair.src.clone(),
            handle,
            packet_queue_tx,
            cancel,
        };
        let client = Client::new(conn);
        self.connections.insert(pair, client);
    }
    pub async fn unregister(&mut self, node_pair: NodePair) {
        if let Some(client) = self.connections.remove(&node_pair) {
            tokio::spawn(async move {
                client.shutdown().await;
            });
        }
    }
}
