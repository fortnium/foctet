use crate::stream::{LogicalStream, StreamEvent, StreamState};
use anyhow::Result;
use foctet_core::{
    codec::FrameCodec, connection::SessionId, frame::{Frame, FrameFlags}, stream::StreamId
};
use futures::{SinkExt, StreamExt};
use nohash_hasher::IntMap;
use std::{marker::PhantomData, net::SocketAddr};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf},
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    time::Interval,
};
use tokio_util::{
    codec::{FramedRead, FramedWrite},
    sync::CancellationToken,
    task::AbortOnDropHandle,
};

/// Session side, client or server
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SessionSide {
    /// The session is a client
    Client,
    /// The session is a server (typical low level stream is an accepted TcpStream)
    Server,
}

impl SessionSide {
    /// If this is a client type (inbound connection)
    pub fn is_client(self) -> bool {
        self == SessionSide::Client
    }

    /// If this is a server type (outbound connection)
    pub fn is_server(self) -> bool {
        self == SessionSide::Server
    }
}

#[derive(Debug)]
pub enum Command {
    OpenStream(oneshot::Sender<Result<LogicalStream>>),
    Shutdown(oneshot::Sender<()>),
}

pub struct SessionActor<T> {
    /// Framed low level raw stream writer
    framed_writer: FramedWrite<WriteHalf<T>, FrameCodec>,
    /// Framed low level raw stream reader
    framed_reader: FramedRead<ReadHalf<T>, FrameCodec>,
    /// Session ID
    session_id: SessionId,
    /// next_stream_id is the next stream we should
    /// send. This depends if we are a client/server.
    next_stream_id: StreamId,
    /// remote_closed indicates the remote side does
    /// not want further connections. Must be first for alignment.
    remote_closed: bool,
    /// local_closed indicates that we should stop
    /// accepting further connections. Must be first for alignment.
    local_closed: bool,
    /// pending_streams maps a stream id to a sender of logical-stream.
    /// waiting for connection_response
    pending_streams: IntMap<StreamId, oneshot::Sender<Result<LogicalStream>>>,
    /// streams maps a stream id to a sender of stream,
    streams: IntMap<StreamId, Sender<Frame>>,
    /// For receive events from sub streams (for clone to new stream)
    event_sender: Sender<StreamEvent>,
    /// For receive events from sub streams
    event_receiver: Receiver<StreamEvent>,
    /// Receive control command from session
    control_receiver: Receiver<Command>,
    /// Send the new incoming logical-stream to the session
    stream_sender: Sender<LogicalStream>,
    /// Keepalive interval
    keepalive: Option<Interval>,
    /// Cancel token
    cancel: CancellationToken,
}

impl<T> SessionActor<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    tracing::info!("SessionActor loop cancelled, closing loop");
                    break;
                }
                Some(frame_result) = self.framed_reader.next() => {
                    match frame_result {
                        Ok(frame) => {
                            if let Err(e) = self.handle_incoming_frame(frame).await {
                                tracing::error!("Error handling incoming frame: {:?}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Framed reader error: {:?}", e);
                            self.remote_closed = true;
                            break;
                        }
                    }
                }
                Some(cmd) = self.control_receiver.recv() => {
                    if let Err(e) = self.handle_control_command(cmd).await {
                        tracing::error!("Error handling control command: {:?}", e);
                        break;
                    }
                }
                Some(event) = self.event_receiver.recv() => {
                    if let Err(e) = self.handle_stream_event(event).await {
                        tracing::error!("Error handling stream event: {:?}", e);
                        break;
                    }
                }
            }
        }

        // Shutdown session
        self.shutdown().await;
    }

    async fn handle_incoming_frame(&mut self, frame: Frame) -> Result<(), anyhow::Error> {
        let stream_id = frame.header.stream_id;

        if let Some(sender) = self.streams.get(&stream_id) {
            // Send the frame to the logical stream
            if let Err(e) = sender.send(frame).await {
                tracing::error!("Failed to send frame to stream {}: {:?}", stream_id.0, e);
                self.streams.remove(&stream_id);
            }
        } else {
            if frame.header.flags.is_open_request() {
                // Create and send new LogicalStream
                let (stream_sender, stream_receiver) = tokio::sync::mpsc::channel(32);

                let logical_stream = LogicalStream::new(
                    self.session_id, 
                    stream_id,
                    StreamState::Established, 
                    self.event_sender.clone(), 
                    stream_receiver
                );

                // Regist new stream to map
                self.streams.insert(stream_id, stream_sender);

                // Send new LogicalStream to session
                if let Err(e) = self.stream_sender.send(logical_stream).await {
                    tracing::error!("Failed to send new stream to session: {:?}", e);
                    self.streams.remove(&stream_id);
                }

                // Send open response to remote
                let open_response_frame = Frame::builder()
                    .with_stream_id(stream_id)
                    .with_flags(FrameFlags::open_response())
                    .build();
                
                if let Err(e) = self.framed_writer.send(open_response_frame).await {
                    tracing::error!("Failed to send open response: {:?}", e);
                }

                tracing::debug!("New stream accepted: {}", stream_id.0);
            } else if frame.header.flags.is_open_response() {
                // Open response received. Send to pending stream.
                if let Some(sender) = self.pending_streams.remove(&stream_id) {
                    let (stream_sender, stream_receiver) = tokio::sync::mpsc::channel(32);
                    // Regist new stream to map
                    self.streams.insert(stream_id, stream_sender);
                    // Send new LogicalStream to waiting channel
                    if let Err(e) = sender.send(Ok(LogicalStream::new(
                        self.session_id, 
                        stream_id,
                        StreamState::Established, 
                        self.event_sender.clone(), 
                        stream_receiver
                    ))) {
                        tracing::error!("Failed to send new LogicalStream: {:?}", e);
                    }
                } else {
                    tracing::error!("Received open response for unknown stream {}", stream_id.0);
                }
            } else if frame.header.flags.is_open_reset() {
                // Open reset received: stream was rejected by remote
                if let Some(sender) = self.pending_streams.remove(&stream_id) {
                    let _ = sender.send(Err(anyhow::anyhow!(
                        "Stream {} rejected by remote", stream_id.0
                    )));
                    tracing::debug!("Stream {} was rejected by remote", stream_id.0);
                } else {
                    tracing::warn!("Received open_reset for unknown pending stream {}", stream_id.0);
                }
            } else {
                // Unknown stream and NOT a open request
                tracing::error!("Received frame for unknown stream {} without open request", stream_id.0);
                // TODO: Should send RESET?
            }
        }

        Ok(())
    }

    async fn handle_control_command(&mut self, cmd: Command) -> Result<(), anyhow::Error> {
        match cmd {
            Command::OpenStream(reply_tx) => {
                // Get new stream ID
                let stream_id = self.next_stream_id.fetch_add(1);
                let (resp_tx, resp_rx) = oneshot::channel();

                // Regist new stream responder to map
                self.pending_streams.insert(stream_id, resp_tx);

                // Send open request to remote
                let open_frame = Frame::builder()
                .with_stream_id(stream_id)
                .with_flags(FrameFlags::open_request())
                .build();

                self.framed_writer.send(open_frame).await?;

                // Wait for open response
                tokio::spawn(async move {
                    match resp_rx.await {
                        Ok(Ok(stream)) => {
                            let _ = reply_tx.send(Ok(stream));
                        }
                        Ok(Err(e)) => {
                            let _ = reply_tx.send(Err(e));
                        }
                        Err(_) => {
                            let _ = reply_tx.send(Err(anyhow::anyhow!("No response received")));
                        }
                    }
                });

                tracing::debug!("New stream opened: {}", stream_id.0);
            }
            Command::Shutdown(reply_tx) => {
                // Set local closed
                self.local_closed = true;
                let _ = reply_tx.send(());
            }
        }

        Ok(())
    }

    async fn handle_stream_event(&mut self, event: StreamEvent) -> Result<(), anyhow::Error> {
        match event {
            StreamEvent::Frame(frame) => {
                // Send the frame via RAW stream
                self.framed_writer.send(frame).await.map_err(|e| {
                    anyhow::anyhow!("Failed to send frame to writer: {:?}", e)
                })?;
            }
            StreamEvent::Closed(stream_id) => {
                // logical-stream closed. Remove from map.
                self.streams.remove(&stream_id);
                tracing::debug!("Stream {} closed and removed", stream_id);
            }
            StreamEvent::Error => {
                // Error from logical-stream.
                // Currently omitted. Only log.
                tracing::warn!("Stream event error received");
                // TODO: handle error
            }
        }
    
        Ok(())
    }

    async fn shutdown(&mut self) {
        tracing::info!("Session {} shutting down", self.session_id);

        // Close all stream
        self.streams.clear();
        tracing::debug!("All logical streams closed");

        // Close framed_writer
        if let Err(e) = self.framed_writer.close().await {
            tracing::warn!("Error while closing framed writer: {:?}", e);
        } else {
            tracing::info!("Framed writer closed successfully");
        }
    }

    pub async fn keepalive_tick(&mut self) {
        if let Some(_keepalive) = &mut self.keepalive {
            // TODO: implement keepalive
        }
    }

}

/// The session
pub struct Session<T> {
    _marker: PhantomData<T>,
    /// Session ID
    session_id: SessionId,
    /// Client or Server
    side: SessionSide,
    /// SessionActor handle
    handle: AbortOnDropHandle<()>,
    /// Send control command to SessionActor
    control_sender: Sender<Command>,
    /// Receive new incoming logical-stream from SessionActor
    stream_receiver: Receiver<LogicalStream>,
    /// Cancel token
    cancel: CancellationToken,
    /// Local socket address
    local_addr: Option<SocketAddr>,
    /// Remote socket address
    remote_addr: Option<SocketAddr>,
}

impl<T> Session<T> 
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub async fn spawn(
        stream: T,
        side: SessionSide,
        session_id: SessionId,
        local_addr: Option<SocketAddr>,
        remote_addr: Option<SocketAddr>,
    ) -> Self {    
        let (read_half, write_half) = tokio::io::split(stream);
        let framed_reader = FramedRead::new(read_half, FrameCodec::new());
        let framed_writer = FramedWrite::new(write_half, FrameCodec::new());
    
        let (event_sender, event_receiver) = mpsc::channel(32);
        let (control_sender, control_receiver) = mpsc::channel(8);
        let (stream_sender, stream_receiver) = mpsc::channel(32);
    
        let next_stream_id = match side {
            SessionSide::Client => StreamId(1),
            SessionSide::Server => StreamId(2),
        };

        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
    
        let actor = SessionActor {
            framed_reader,
            framed_writer,
            session_id,
            next_stream_id,
            remote_closed: false,
            local_closed: false,
            pending_streams: IntMap::default(),
            streams: IntMap::default(),
            event_sender: event_sender.clone(),
            event_receiver,
            control_receiver,
            stream_sender: stream_sender.clone(),
            keepalive: None,
            cancel: cancel_clone,
        };
    
        let handle = tokio::spawn(async move {
            actor.run().await;
        });
    
        let handle = AbortOnDropHandle::new(handle);
    
        Session {
            _marker: PhantomData,
            session_id,
            side,
            handle,
            control_sender,
            stream_receiver,
            cancel,
            local_addr,
            remote_addr,
        }
    }
    pub async fn new_client(raw_stream: T, session_id: SessionId) -> Self {
        Self::spawn(raw_stream, SessionSide::Client, session_id, None, None).await
    }
    pub async fn new_server(raw_stream: T, session_id: SessionId) -> Self {
        Self::spawn(raw_stream, SessionSide::Server, session_id, None, None).await
    }
    pub async fn open_stream(&self) -> Result<LogicalStream, anyhow::Error> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        self.control_sender.send(Command::OpenStream(reply_tx)).await.map_err(|e| {
            anyhow::anyhow!("Failed to send OpenStream command: {:?}", e)
        })?;

        match reply_rx.await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => Err(anyhow::anyhow!("Stream open failed: {:?}", e)),
            Err(e) => Err(anyhow::anyhow!("Stream open response failed: {:?}", e)),
        }
    }

    pub async fn accept_stream(&mut self) -> Result<LogicalStream, anyhow::Error> {
        match self.stream_receiver.recv().await {
            Some(stream) => Ok(stream),
            None => Err(anyhow::anyhow!("Session closed")),
        }
    }

    pub async fn shutdown(&self) -> Result<(), anyhow::Error> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        self.control_sender.send(Command::Shutdown(reply_tx)).await.map_err(|e| {
            anyhow::anyhow!("Failed to send Shutdown command: {:?}", e)
        })?;

        let _ = reply_rx.await;
        self.cancel.cancel();
        Ok(())
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub fn side(&self) -> SessionSide {
        self.side
    }

    pub fn is_active(&self) -> bool {
        !self.handle.is_finished()
    }

    pub fn set_local_addr(&mut self, addr: SocketAddr) {
        self.local_addr = Some(addr);
    }

    pub fn set_remote_addr(&mut self, addr: SocketAddr) {
        self.remote_addr = Some(addr);
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

}
