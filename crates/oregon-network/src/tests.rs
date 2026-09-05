use std::{net::SocketAddr, time::Duration};

use oregon_protocol::{
    FRAME_HEADER_BYTES, FRAME_VERSION, MAX_FRAME_PAYLOAD, Message, ProtocolError, TAG_PING,
    build_frame_header, encode_message,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::{advance, timeout},
};

use crate::{
    FramedConnection, NetworkError, TcpTransport, Transport, TransportConnection, TransportListener,
};

const MAGIC: [u8; 4] = [0xca, 0x20, 0x34, 0xec];

fn remote_addr() -> SocketAddr {
    "127.0.0.1:18444".parse().unwrap()
}

fn encoded_frame(message: &Message) -> Vec<u8> {
    let (message_type, payload) = encode_message(message).unwrap();
    let header = build_frame_header(MAGIC, message_type, &payload).unwrap();
    let mut frame = header.encode().to_vec();
    frame.extend_from_slice(&payload);
    frame
}

async fn assert_connection_roundtrip<C, S>(
    client: &mut C,
    server: &mut S,
    listening_addr: SocketAddr,
) where
    C: TransportConnection,
    S: TransportConnection,
{
    assert_eq!(client.remote_addr(), listening_addr);
    client.write_message(&Message::Ping(17)).await.unwrap();
    assert_eq!(server.read_message().await.unwrap(), Message::Ping(17));
    client.shutdown().await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn oversized_header_is_rejected_before_waiting_for_payload() {
    let (mut writer, stream) = tokio::io::duplex(FRAME_HEADER_BYTES);
    let mut header = [0u8; FRAME_HEADER_BYTES];
    header[..4].copy_from_slice(&MAGIC);
    header[4] = FRAME_VERSION;
    header[5] = TAG_PING;
    header[8..12].copy_from_slice(&((MAX_FRAME_PAYLOAD + 1) as u32).to_le_bytes());
    writer.write_all(&header).await.unwrap();

    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);
    let result = timeout(Duration::from_millis(100), connection.read_message())
        .await
        .expect("reader waited for an oversized payload");

    assert!(matches!(
        result,
        Err(NetworkError::OversizedFrame {
            declared,
            max: MAX_FRAME_PAYLOAD,
        }) if declared == (MAX_FRAME_PAYLOAD + 1) as u32
    ));
}

#[tokio::test]
async fn wrong_network_magic_is_rejected_before_waiting_for_payload() {
    let (mut writer, stream) = tokio::io::duplex(FRAME_HEADER_BYTES);
    let header = build_frame_header([0, 0, 0, 0], TAG_PING, &[0; 8]).unwrap();
    writer.write_all(&header.encode()).await.unwrap();
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);

    let result = timeout(Duration::from_millis(100), connection.read_message())
        .await
        .expect("reader waited for a payload from the wrong network");

    assert!(matches!(
        result,
        Err(NetworkError::Protocol(ProtocolError::WrongNetworkMagic))
    ));
}

#[tokio::test]
async fn framed_connection_round_trips_a_protocol_message() {
    let (left, right) = tokio::io::duplex(64);
    let mut sender = FramedConnection::new(left, remote_addr(), MAGIC);
    let mut receiver = FramedConnection::new(right, remote_addr(), MAGIC);
    let expected = oregon_protocol::Message::Ping(0x0102_0304_0506_0708);

    sender.write_message(&expected).await.unwrap();

    assert_eq!(receiver.read_message().await.unwrap(), expected);
}

#[tokio::test]
async fn corrupted_payload_is_rejected_by_checksum() {
    let (mut writer, stream) = tokio::io::duplex(64);
    let mut frame = encoded_frame(&Message::Ping(7));
    frame[FRAME_HEADER_BYTES] ^= 1;
    writer.write_all(&frame).await.unwrap();
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);

    assert!(matches!(
        connection.read_message().await,
        Err(NetworkError::Protocol(ProtocolError::ChecksumMismatch))
    ));
}

#[tokio::test]
async fn truncated_payload_reports_received_and_expected_bytes() {
    let (mut writer, stream) = tokio::io::duplex(64);
    let frame = encoded_frame(&Message::Ping(9));
    writer.write_all(&frame[..frame.len() - 1]).await.unwrap();
    writer.shutdown().await.unwrap();
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);

    assert!(matches!(
        connection.read_message().await,
        Err(NetworkError::TruncatedFrame {
            received: 7,
            expected: 8,
        })
    ));
}

#[tokio::test(start_paused = true)]
async fn read_fails_after_fifteen_seconds_without_progress() {
    let (_writer, stream) = tokio::io::duplex(64);
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);
    let read = tokio::spawn(async move { connection.read_message().await });
    tokio::task::yield_now().await;

    advance(Duration::from_secs(14)).await;
    assert!(!read.is_finished());
    advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(read.is_finished(), "read survived the 15 second boundary");

    assert!(matches!(
        read.await.unwrap(),
        Err(NetworkError::ReadNoProgressTimeout)
    ));
}

#[tokio::test(start_paused = true)]
async fn trickle_bytes_cannot_extend_read_beyond_sixty_seconds() {
    let (mut writer, stream) = tokio::io::duplex(64);
    let frame = encoded_frame(&Message::Ping(11));
    writer
        .write_all(&frame[..FRAME_HEADER_BYTES])
        .await
        .unwrap();
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);
    let read = tokio::spawn(async move { connection.read_message().await });
    tokio::task::yield_now().await;

    for byte in &frame[FRAME_HEADER_BYTES..FRAME_HEADER_BYTES + 5] {
        advance(Duration::from_secs(10)).await;
        writer.write_all(&[*byte]).await.unwrap();
        tokio::task::yield_now().await;
        assert!(!read.is_finished());
    }
    advance(Duration::from_secs(9)).await;
    assert!(!read.is_finished());
    advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(read.is_finished(), "read survived the 60 second boundary");

    let result = read.await.unwrap();
    assert!(
        matches!(result, Err(NetworkError::ReadDeadlineExceeded)),
        "unexpected read result: {result:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn blocked_write_fails_after_fifteen_seconds() {
    let (stream, _reader) = tokio::io::duplex(1);
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);
    let write = tokio::spawn(async move { connection.write_message(&Message::Ping(13)).await });
    tokio::task::yield_now().await;

    advance(Duration::from_secs(14)).await;
    assert!(!write.is_finished());
    advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(write.is_finished(), "write survived the 15 second boundary");

    assert!(matches!(
        write.await.unwrap(),
        Err(NetworkError::WriteDeadlineExceeded)
    ));
}

#[tokio::test(start_paused = true)]
async fn intermittent_write_progress_cannot_extend_fifteen_second_deadline() {
    let (stream, mut reader) = tokio::io::duplex(1);
    let mut connection = FramedConnection::new(stream, remote_addr(), MAGIC);
    let write = tokio::spawn(async move { connection.write_message(&Message::Ping(15)).await });
    tokio::task::yield_now().await;

    for _ in 0..2 {
        advance(Duration::from_secs(5)).await;
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).await.unwrap();
        tokio::task::yield_now().await;
        assert!(!write.is_finished());
    }
    advance(Duration::from_secs(4)).await;
    assert!(!write.is_finished());
    advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(write.is_finished(), "write progress extended the deadline");

    assert!(matches!(
        write.await.unwrap(),
        Err(NetworkError::WriteDeadlineExceeded)
    ));
}

#[tokio::test]
async fn tcp_transport_binds_connects_and_round_trips() {
    let transport = TcpTransport;
    let mut listener = transport
        .bind("127.0.0.1:0".parse().unwrap(), MAGIC)
        .await
        .unwrap();
    let listening_addr = listener.local_addr();

    let (client, server) =
        tokio::join!(transport.connect(listening_addr, MAGIC), listener.accept());
    let mut client = client.unwrap();
    let mut server = server.unwrap();
    assert_connection_roundtrip(&mut client, &mut server, listening_addr).await;
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn outbound_remote_addr_uses_kernel_observed_endpoint() {
    let transport = TcpTransport;
    let mut listener = transport
        .bind("127.0.0.1:0".parse().unwrap(), MAGIC)
        .await
        .unwrap();
    let listening_addr = listener.local_addr();
    let requested_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), listening_addr.port());

    let (client, server) =
        tokio::join!(transport.connect(requested_addr, MAGIC), listener.accept());
    let mut client = client.unwrap();
    let mut server = server.unwrap();

    assert!(client.remote_addr().ip().is_loopback());
    client.shutdown().await.unwrap();
    server.shutdown().await.unwrap();
}
