use anyhow::Result;
use bytes::Bytes;
use foctet_core::connection::SessionId;
use foctet_core::frame::{Frame, FrameBuilder, FrameFlags, FrameType};
use foctet_core::stream::StreamId;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc::{Receiver, Sender};

/// The stream state
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StreamState {
    /// Just created
    Init,
    /// We sent a open request message
    OpenSent,
    /// We received a open request message
    OpenReceived,
    /// Stream established
    Established,
    /// We closed the stream
    LocalClosing,
    /// Remote closed the stream
    RemoteClosing,
    /// Both side of the stream closed
    Closed,
    /// Stream rejected by remote
    Reset,
}

// Stream event
#[derive(Debug, Eq, PartialEq)]
pub enum StreamEvent {
    Frame(Frame),
    Closed(StreamId),
    Error,
}

/// Writer half of LogicalStream
#[derive(Debug)]
pub struct LogicalStreamWriter {
    session_id: SessionId,
    stream_id: StreamId,
    state: Arc<Mutex<StreamState>>,
    frame_sender: Sender<StreamEvent>,
}

impl LogicalStreamWriter {
    pub fn set_state(&mut self, state: StreamState) {
        match self.state.lock() {
            Ok(mut state_guard) => {
                *state_guard = state;
            }
            Err(_) => (),
        }
    }
    /// Get the session id
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    /// Get the stream id
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    /// Get the stream state
    pub fn state(&self) -> StreamState {
        match self.state.lock() {
            Ok(state_guard) => *state_guard,
            Err(_) => StreamState::Closed,
        }
    }
    pub async fn send_event(&self, event: StreamEvent) -> Result<()> {
        match self.frame_sender.send(event).await {
            Ok(_) => Ok(()),
            Err(_) => anyhow::bail!(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to send event"
            )),
        }
    }
    /// Send a frame
    pub async fn send_frame(&self, frame: Frame) -> Result<()> {
        self.frame_sender
            .send(StreamEvent::Frame(frame))
            .await
            .map_err(|_| {
                anyhow::anyhow!(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Failed to send frame"
                ))
            })
    }

    /// Send raw bytes as a data frame
    pub async fn send_bytes(&self, bytes: Bytes) -> Result<()> {
        let frame = FrameBuilder::new()
            .with_stream_id(self.stream_id)
            .with_frame_type(FrameType::Data)
            .with_payload(bytes)
            .build();
        self.send_frame(frame).await
    }

    async fn send_close_request(&mut self) -> Result<()> {
        let frame_flags = FrameFlags::close_request();
        let close_frame: Frame = Frame::builder()
            .with_stream_id(self.stream_id)
            .with_flags(frame_flags)
            .build();
        match self
            .frame_sender
            .send(StreamEvent::Frame(close_frame))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => anyhow::bail!(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to send close frame"
            )),
        }
    }

    /// Close the stream
    pub async fn close(&mut self) -> Result<()> {
        let state = match self.state.lock() {
            Ok(state_guard) => *state_guard,
            Err(_) => StreamState::Closed,
        };
        match state {
            StreamState::OpenSent
            | StreamState::OpenReceived
            | StreamState::Established
            | StreamState::Init => {
                self.set_state(StreamState::LocalClosing);
                self.send_close_request().await?;
            }
            StreamState::RemoteClosing => {
                self.set_state(StreamState::Closed);
                self.send_close_request().await?;
                let event = StreamEvent::Closed(self.stream_id);
                self.send_event(event).await?;
            }
            StreamState::Reset | StreamState::Closed => {
                self.set_state(StreamState::Closed);
                let event = StreamEvent::Closed(self.stream_id);
                self.send_event(event).await?;
            }
            StreamState::LocalClosing => {
                self.set_state(StreamState::Closed);
                let event = StreamEvent::Closed(self.stream_id);
                self.send_event(event).await?;
            }
        }
        Ok(())
    }
}

impl AsyncWrite for LogicalStreamWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let payload = Bytes::copy_from_slice(buf);
        let frame = FrameBuilder::new()
            .with_stream_id(self.stream_id)
            .with_frame_type(FrameType::Data)
            .with_payload(payload)
            .build();

        match self.frame_sender.try_send(StreamEvent::Frame(frame)) {
            Ok(_) => {
                Poll::Ready(Ok(buf.len()))
            }
            Err(_) => {
                tracing::error!("Failed to send frame");
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Failed to send frame",
                )));
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        // TODO: closing the logical stream here.
        Poll::Ready(Ok(()))
    }
}

/// Reader half of LogicalStream
#[derive(Debug)]
pub struct LogicalStreamReader {
    session_id: SessionId,
    stream_id: StreamId,
    state: Arc<Mutex<StreamState>>,
    frame_receiver: Receiver<Frame>,
}

impl LogicalStreamReader {
    pub fn set_state(&mut self, state: StreamState) {
        match self.state.lock() {
            Ok(mut state_guard) => {
                *state_guard = state;
            }
            Err(_) => (),
        }
    }
    /// Get the session id
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    /// Get the stream id
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    /// Get the stream state
    pub fn state(&self) -> StreamState {
        match self.state.lock() {
            Ok(state_guard) => *state_guard,
            Err(_) => StreamState::Closed,
        }
    }
    /// Receive a frame
    pub async fn recv_frame(&mut self) -> Result<Frame> {
        match self.frame_receiver.recv().await {
            Some(frame) => {
                self.set_state_from_flags(frame.header.flags);
                Ok(frame)
            }
            None => {
                if self.frame_receiver.is_closed() {
                    anyhow::bail!(io::Error::new(io::ErrorKind::BrokenPipe, "Channel closed"))
                } else {
                    anyhow::bail!(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Failed to receive frame"
                    ))
                }
            }
        }
    }

    /// Receive raw bytes from a data frame
    pub async fn recv_bytes(&mut self) -> Result<Bytes> {
        let frame = self.recv_frame().await?;
        Ok(frame.payload)
    }

    fn set_state_from_flags(&mut self, flags: FrameFlags) {
        if flags.is_open_request() {
            self.set_state(StreamState::OpenReceived);
        } else if flags.is_open_response() {
            self.set_state(StreamState::Established);
        } else if flags.is_open_reset() {
            self.set_state(StreamState::Reset);
        }
    }
}

impl AsyncRead for LogicalStreamReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.frame_receiver.try_recv() {
            Ok(frame) => {
                self.set_state_from_flags(frame.header.flags);
                buf.put_slice(&frame.payload);
                Poll::Ready(Ok(()))
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => Poll::Pending,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                // Session closed, mark as EOF
                self.set_state(StreamState::Closed);
                Poll::Ready(Ok(()))
            }
        }
    }
}

#[derive(Debug)]
pub struct LogicalStream {
    /// Session ID
    session_id: SessionId,
    /// Stream ID
    stream_id: StreamId,
    /// Stream state
    state: StreamState,
    // Send frame to parent session
    frame_sender: Sender<StreamEvent>,
    // Receive frame of current stream from parent session
    // (if the sender closed means session closed the stream should close too)
    frame_receiver: Receiver<Frame>,
}

impl LogicalStream {
    pub fn new(
        session_id: SessionId,
        stream_id: StreamId,
        state: StreamState,
        frame_sender: Sender<StreamEvent>,
        frame_receiver: Receiver<Frame>,
    ) -> Self {
        Self {
            session_id,
            stream_id,
            state,
            frame_sender,
            frame_receiver,
        }
    }

    pub async fn send_frame(&self, frame: Frame) -> Result<()> {
        match self.frame_sender.send(StreamEvent::Frame(frame)).await {
            Ok(_) => Ok(()),
            Err(_) => anyhow::bail!(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to send frame"
            )),
        }
    }

    pub async fn recv_frame(&mut self) -> Result<Frame> {
        match self.frame_receiver.recv().await {
            Some(frame) => {
                self.set_state_from_flags(frame.header.flags);
                Ok(frame)
            }
            None => anyhow::bail!(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to receive frame"
            )),
        }
    }

    async fn send_event(&self, event: StreamEvent) -> Result<()> {
        match self.frame_sender.send(event).await {
            Ok(_) => Ok(()),
            Err(_) => anyhow::bail!(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to send event"
            )),
        }
    }

    async fn send_close_request(&mut self) -> Result<()> {
        let frame_flags = FrameFlags::close_request();
        let close_frame: Frame = Frame::builder()
            .with_stream_id(self.stream_id)
            .with_flags(frame_flags)
            .build();
        match self
            .frame_sender
            .send(StreamEvent::Frame(close_frame))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => anyhow::bail!(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Failed to send close frame"
            )),
        }
    }

    /// Close the stream
    pub async fn close(&mut self) -> Result<()> {
        match self.state {
            StreamState::OpenSent
            | StreamState::OpenReceived
            | StreamState::Established
            | StreamState::Init => {
                self.state = StreamState::LocalClosing;
                self.send_close_request().await?;
            }
            StreamState::RemoteClosing => {
                self.state = StreamState::Closed;
                self.send_close_request().await?;
                let event = StreamEvent::Closed(self.stream_id);
                self.send_event(event).await?;
            }
            StreamState::Reset | StreamState::Closed => {
                self.state = StreamState::Closed;
                let event = StreamEvent::Closed(self.stream_id);
                self.send_event(event).await?;
            }
            StreamState::LocalClosing => {
                self.state = StreamState::Closed;
                let event = StreamEvent::Closed(self.stream_id);
                self.send_event(event).await?;
            }
        }
        Ok(())
    }

    fn set_state_from_flags(&mut self, flags: FrameFlags) {
        if flags.is_open_request() {
            self.state = StreamState::OpenReceived;
        } else if flags.is_open_response() {
            self.state = StreamState::Established;
        } else if flags.is_open_reset() {
            self.state = StreamState::Reset;
        } else if flags.is_close_request() {
            self.state = StreamState::RemoteClosing;
        } else if flags.is_close_response() {
            self.state = StreamState::Closed;
        }
    }

    /// Split this stream into a reader and writer
    pub fn split(self) -> (LogicalStreamWriter, LogicalStreamReader) {
        let state = Arc::new(Mutex::new(self.state));
        let writer = LogicalStreamWriter {
            session_id: self.session_id,
            stream_id: self.stream_id,
            state: Arc::clone(&state),
            frame_sender: self.frame_sender,
        };
        let reader = LogicalStreamReader {
            session_id: self.session_id,
            stream_id: self.stream_id,
            state,
            frame_receiver: self.frame_receiver,
        };
        (writer, reader)
    }

    pub fn set_state(&mut self, state: StreamState) {
        self.state = state;
    }

    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    pub fn state(&self) -> StreamState {
        self.state
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
}
