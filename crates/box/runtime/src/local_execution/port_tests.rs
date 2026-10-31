#![cfg(test)]

use std::num::NonZeroU16;
use std::time::Duration;

use aster_box_core::{ExecutionId, ExecutionManagerError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

use super::{
    connect_microvm_port, connect_microvm_udp_port, read_port_forward_frame,
    write_port_forward_frame, PORT_FORWARD_FRAME_CLOSE, PORT_FORWARD_FRAME_DATA,
    PORT_FORWARD_FRAME_OPEN, PORT_FORWARD_FRAME_OPEN_ACK, PORT_FORWARD_FRAME_OPEN_UDP,
    PORT_FORWARD_STREAM_ID,
};

#[tokio::test]
async fn microvm_udp_channel_preserves_datagrams_and_receive_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("udp.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let guest = tokio::spawn(async move {
        let (mut control, _) = listener.accept().await.unwrap();
        let open = read_port_forward_frame(&mut control)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(open.kind, PORT_FORWARD_FRAME_OPEN_UDP);
        assert_eq!(open.payload, 41000_u16.to_be_bytes());
        write_port_forward_frame(
            &mut control,
            PORT_FORWARD_FRAME_OPEN_ACK,
            PORT_FORWARD_STREAM_ID,
            &[0],
        )
        .await
        .unwrap();
        for expected in [b"first".as_slice(), b"second".as_slice()] {
            let frame = read_port_forward_frame(&mut control)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(frame.kind, PORT_FORWARD_FRAME_DATA);
            assert_eq!(frame.stream_id, PORT_FORWARD_STREAM_ID);
            assert_eq!(frame.payload, expected);
            write_port_forward_frame(
                &mut control,
                PORT_FORWARD_FRAME_DATA,
                PORT_FORWARD_STREAM_ID,
                expected,
            )
            .await
            .unwrap();
        }
        write_port_forward_frame(
            &mut control,
            PORT_FORWARD_FRAME_CLOSE,
            PORT_FORWARD_STREAM_ID,
            &[],
        )
        .await
        .unwrap();
    });
    let id = ExecutionId::new("microvm-udp-relay").unwrap();
    let mut port = connect_microvm_udp_port(
        &id,
        &path,
        NonZeroU16::new(41000).unwrap(),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    port.send_datagram(b"first").await.unwrap();
    port.send_datagram(b"second").await.unwrap();
    assert_eq!(port.recv_datagram(5).await.unwrap(), b"first");
    assert_eq!(
        port.recv_datagram(5).await.unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    guest.await.unwrap();
}

#[tokio::test]
async fn microvm_port_channel_relays_bidirectional_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let socket_path = directory.path().join("portfwd.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();
    let guest = tokio::spawn(async move {
        let (mut control, _) = listener.accept().await.unwrap();
        let open = read_port_forward_frame(&mut control)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(open.kind, PORT_FORWARD_FRAME_OPEN);
        assert_eq!(open.stream_id, PORT_FORWARD_STREAM_ID);
        assert_eq!(open.payload, 8_080_u16.to_be_bytes());
        write_port_forward_frame(
            &mut control,
            PORT_FORWARD_FRAME_OPEN_ACK,
            PORT_FORWARD_STREAM_ID,
            &[0],
        )
        .await
        .unwrap();

        let request = read_port_forward_frame(&mut control)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(request.kind, PORT_FORWARD_FRAME_DATA);
        assert_eq!(request.stream_id, PORT_FORWARD_STREAM_ID);
        assert_eq!(request.payload, b"host-to-guest");
        write_port_forward_frame(
            &mut control,
            PORT_FORWARD_FRAME_DATA,
            PORT_FORWARD_STREAM_ID,
            b"guest-to-host",
        )
        .await
        .unwrap();
        write_port_forward_frame(
            &mut control,
            PORT_FORWARD_FRAME_CLOSE,
            PORT_FORWARD_STREAM_ID,
            &[],
        )
        .await
        .unwrap();
    });

    let execution_id = ExecutionId::new("microvm-port-relay").unwrap();
    let mut stream = connect_microvm_port(
        &execution_id,
        &socket_path,
        NonZeroU16::new(8_080).unwrap(),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    stream.write_all(b"host-to-guest").await.unwrap();
    let mut response = [0_u8; 13];
    stream.read_exact(&mut response).await.unwrap();
    assert_eq!(&response, b"guest-to-host");
    let mut eof = [0_u8; 1];
    assert_eq!(stream.read(&mut eof).await.unwrap(), 0);
    guest.await.unwrap();
}

#[tokio::test]
async fn microvm_port_channel_reports_guest_rejection() {
    let directory = tempfile::tempdir().unwrap();
    let socket_path = directory.path().join("portfwd.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();
    let guest = tokio::spawn(async move {
        let (mut control, _) = listener.accept().await.unwrap();
        let _ = read_port_forward_frame(&mut control).await.unwrap();
        write_port_forward_frame(
            &mut control,
            PORT_FORWARD_FRAME_OPEN_ACK,
            PORT_FORWARD_STREAM_ID,
            &[1],
        )
        .await
        .unwrap();
    });

    let execution_id = ExecutionId::new("microvm-port-rejected").unwrap();
    let error = match connect_microvm_port(
        &execution_id,
        &socket_path,
        NonZeroU16::new(8_080).unwrap(),
        Duration::from_secs(2),
    )
    .await
    {
        Ok(_) => panic!("guest rejection must fail the connection"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        ExecutionManagerError::Unavailable(message)
            if message.contains("rejected") && message.contains("8080")
    ));
    guest.await.unwrap();
}

#[tokio::test]
async fn microvm_port_channel_rejects_oversized_guest_frames() {
    let (mut writer, mut reader) = tokio::io::duplex(32);
    writer
        .write_all(&[PORT_FORWARD_FRAME_DATA, 0, 0, 0, 1, 0, 1, 0, 1])
        .await
        .unwrap();
    let error = read_port_forward_frame(&mut reader).await.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
