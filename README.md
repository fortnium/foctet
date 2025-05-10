# [WIP] foctet
Foctet is a framework for secure and reliable peer-to-peer (P2P) communication.  
Implemented in Rust, Foctet leverages QUIC, TCP (TLS), and multiplexing techniques to deliver exceptional throughput and minimal latency.

## Features
- Multiple Transport Protocols: Supports QUIC and TCP (TLS) for flexible communication.
- Secure: Authenticated end-to-end encryption using TLS 1.3 for both QUIC and TCP/TLS connections
- Multiplexing: Efficiently manages multiple streams over a single connection.
- Reliable: Direct connection or fall back to Relay.
- NAT Traversal & Relays: Seamless communication across NAT and firewalls using relay servers.

## Architecture
Foctet is structured around a modular design:
* `foctet-core`: Core data structures and frame handling.
* `foctet-net`: Networking and transport layer implementation.
* `foctet-mux`: Multiplexing and logical stream management.
* `foctet-relay`: Relay server and NAT traversal.
* `foctet-cli`: Command-line interface for sending and receiving content.

### End-to-End Encryption
Foctet implements **end-to-end encryption (E2EE)** using TLS 1.3 for both QUIC and TCP/TLS connections.  
All communications between peers are secured using `rustls`, providing robust confidentiality and integrity.

- Self-Signed Certificates:
    - Each node uses a self-signed certificate to verify its identity.
    - The certificate includes the `NodeId`, ensuring that the peer's identity is verified.
- Verifying Node Identity
    - During the TLS handshake, Foctet verifies that the `NodeId` derived from the certificate matches the intended `NodeId`.
- Authentication:
    - Both client and server verify each other's certificate to ensure that the expected `NodeId` matches the certificate content.
