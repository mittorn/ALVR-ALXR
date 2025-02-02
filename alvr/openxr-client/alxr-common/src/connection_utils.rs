use crate::APP_CONFIG;
use alvr_common::prelude::*;
use alvr_common::ALVR_NAME;
use alvr_common::ALVR_VERSION;
use alvr_common::hash_string;
use alvr_sockets::{
    ClientHandshakePacket, HandshakePacket, ServerHandshakePacket, CONTROL_PORT, LOCAL_IP,
    MAX_HANDSHAKE_PACKET_SIZE_BYTES,
};
use std::{net::Ipv4Addr, time::Duration};
use tokio::{net::UdpSocket, time};


pub fn protocol_id() -> u64 {
	18166762639281986762
/*    let protocol_string = if ALVR_VERSION.pre.is_empty() {
        ALVR_VERSION.major.to_string()
    } else {
        format!("{}-{}", ALVR_VERSION.major, ALVR_VERSION.pre)
    };

    hash_string(&protocol_string)*/
}


const CLIENT_HANDSHAKE_RESEND_INTERVAL: Duration = Duration::from_secs(1);

pub enum ConnectionError {
    ServerMessage(ServerHandshakePacket),
    NetworkUnreachable,
}

pub async fn announce_client_loop(
    handshake_packet: ClientHandshakePacket,
) -> StrResult<ConnectionError> {
    println!("announce_client_loop");
    println!("is localhost? {0}", APP_CONFIG.localhost);

    let control_port = if APP_CONFIG.localhost {
        CONTROL_PORT + 1
    } else {
        CONTROL_PORT
    };
    let mut handshake_socket = trace_err!(UdpSocket::bind((LOCAL_IP, control_port)).await)?;
    trace_err!(handshake_socket.set_broadcast(true))?;

    //let client_handshake_packet = trace_err!(bincode::serialize(&HandshakePacket::Client(
     //   handshake_packet
    //)))?;
        let mut packet = [0; 56];
        packet[0..ALVR_NAME.len()].copy_from_slice(ALVR_NAME.as_bytes());
        packet[16..24].copy_from_slice(&protocol_id().to_le_bytes());
        packet[24..24 + handshake_packet.hostname.len()].copy_from_slice(handshake_packet.hostname.as_bytes());


    loop {
        let broadcast_result = handshake_socket
            .send_to(
                &packet,
                (Ipv4Addr::BROADCAST, CONTROL_PORT),
            )
            .await;
        if broadcast_result.is_err() {
            break Ok(ConnectionError::NetworkUnreachable);
        }

        let receive_response_loop = {
            let handshake_socket = &mut handshake_socket;
            async move {
                let mut server_response_buffer = [0; MAX_HANDSHAKE_PACKET_SIZE_BYTES];
                loop {
                    // this call will receive also the broadcasted client packet that must be ignored
                    let (packet_size, _) = trace_err!(
                        handshake_socket
                            .recv_from(&mut server_response_buffer)
                            .await
                    )?;

                    if let Ok(HandshakePacket::Server(handshake_packet)) =
                        bincode::deserialize(&server_response_buffer[..packet_size])
                    {
                        warn!("received packet {:?}", &handshake_packet);
                        println!("received packet {:?}", &handshake_packet);
                        break Ok(ConnectionError::ServerMessage(handshake_packet));
                    }
                }
            }
        };

        tokio::select! {
            res = receive_response_loop => break res,
            _ = time::sleep(CLIENT_HANDSHAKE_RESEND_INTERVAL) => {
                warn!("Server not found, resending handhake packet");
                println!("Server not found, resending handhake packet");
            }
        }
    }
}
