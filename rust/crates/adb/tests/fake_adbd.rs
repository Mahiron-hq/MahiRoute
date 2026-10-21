// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! The client against a stand-in for `adbd`'s pairing service and wireless-debugging transport.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use boring::ssl::{SslAcceptor, SslMethod, SslStream, SslVerifyMode, SslVersion};
use mahiroute_adb::pairing::{
    CLIENT_NAME, EXPORTED_KEY_LABEL, EXPORTED_KEY_LEN, PEER_INFO_LEN, PacketKind, PairingCipher,
    SERVER_NAME, peer_info_record, read_packet, write_packet,
};
use mahiroute_adb::spake2::{Role, Spake2};
use mahiroute_adb::wire::{A_CLSE, A_CNXN, A_OKAY, A_OPEN, A_STLS, A_WRTE, MAX_PAYLOAD, Message};
use mahiroute_adb::{AdbError, AdbKey, Session, pair, run_shell};

fn tls_server(identity: &AdbKey) -> SslAcceptor {
    tls_server_with(identity, true)
}

fn tls_server_with(identity: &AdbKey, accept_clients: bool) -> SslAcceptor {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).expect("acceptor");
    builder
        .set_min_proto_version(Some(SslVersion::TLS1_3))
        .expect("min version");
    builder
        .set_max_proto_version(Some(SslVersion::TLS1_3))
        .expect("max version");
    builder
        .set_certificate(identity.certificate())
        .expect("certificate");
    builder
        .set_private_key(identity.private_key())
        .expect("private key");
    builder.set_verify_callback(
        SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT,
        move |_, _| accept_clients,
    );
    builder.build()
}

/// What the stand-in pairing server learned from one client.
struct PairingOutcome {
    registered_key: Option<Vec<u8>>,
}

fn spawn_pairing_server(code: &'static str) -> (SocketAddr, JoinHandle<PairingOutcome>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let identity = AdbKey::generate().expect("server identity");
        let mut tls = tls_server(&identity).accept(stream).expect("tls accept");

        let mut exported = [0u8; EXPORTED_KEY_LEN];
        tls.ssl()
            .export_keying_material(&mut exported, EXPORTED_KEY_LABEL, None)
            .expect("export");
        let mut password = code.as_bytes().to_vec();
        password.extend_from_slice(&exported);

        let mut spake = Spake2::new(Role::Bob, SERVER_NAME, CLIENT_NAME).expect("spake");
        let reply = spake.generate_message(&password).expect("server message");
        let Ok((PacketKind::Spake2Message, their_message)) = read_packet(&mut tls) else {
            return PairingOutcome {
                registered_key: None,
            };
        };
        write_packet(&mut tls, PacketKind::Spake2Message, &reply).expect("send message");
        let mut cipher = PairingCipher::new(&spake.process_message(&their_message).expect("key"))
            .expect("cipher");

        let Ok((PacketKind::PeerInfo, sealed)) = read_packet(&mut tls) else {
            return PairingOutcome {
                registered_key: None,
            };
        };
        let Ok(record) = cipher.open(&sealed) else {
            // A wrong code makes the record unreadable; adbd hangs up.
            return PairingOutcome {
                registered_key: None,
            };
        };
        assert_eq!(record.len(), PEER_INFO_LEN);
        let key = record[1..]
            .split(|byte| *byte == 0)
            .next()
            .expect("key line")
            .to_vec();
        let answer = peer_info_record(1, b"stand-in-device-guid").expect("record");
        write_packet(
            &mut tls,
            PacketKind::PeerInfo,
            &cipher.seal(&answer).expect("seal"),
        )
        .expect("send info");
        PairingOutcome {
            registered_key: Some(key),
        }
    });
    (address, handle)
}

#[test]
fn pairing_with_the_right_code_registers_the_clients_key() {
    let key = AdbKey::generate().expect("client key");
    let (address, server) = spawn_pairing_server("123456");
    pair(address, "123456", &key, "mahiroute@test").expect("pairing succeeds");

    let outcome = server.join().expect("server thread");
    let registered =
        String::from_utf8(outcome.registered_key.expect("a key was registered")).expect("utf-8");
    let expected = key.public_key_line("mahiroute@test").expect("line");
    assert_eq!(registered.as_bytes(), &expected[..expected.len() - 1]);
}

#[test]
fn pairing_with_a_wrong_code_fails_and_registers_nothing() {
    let key = AdbKey::generate().expect("client key");
    let (address, server) = spawn_pairing_server("123456");
    let error = pair(address, "654321", &key, "mahiroute@test").expect_err("pairing must fail");
    assert!(
        matches!(error, AdbError::Pairing(_)),
        "unexpected error: {error}"
    );
    assert!(
        server
            .join()
            .expect("server thread")
            .registered_key
            .is_none()
    );
}

#[test]
fn pairing_trims_whitespace_around_the_code() {
    let key = AdbKey::generate().expect("client key");
    let (address, server) = spawn_pairing_server("123456");
    pair(address, " 123456\n", &key, "mahiroute@test").expect("pairing succeeds");
    assert!(
        server
            .join()
            .expect("server thread")
            .registered_key
            .is_some()
    );
}

/// Serves one connection like wireless-debugging `adbd`: TLS upgrade, banner, shell streams.
fn spawn_adbd(
    shell_output: &'static [u8],
    peer_key_sink: mpsc::Sender<Vec<u8>>,
) -> (SocketAddr, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let hello = Message::read_from(&mut stream).expect("connection request");
        assert_eq!(hello.command, A_CNXN);
        Message::new(A_STLS, 0x0100_0000, 0)
            .write_to(&mut stream)
            .expect("offer tls");
        let reply = Message::read_from(&mut stream).expect("tls reply");
        assert_eq!((reply.command, reply.arg0), (A_STLS, 0x0100_0000));

        let identity = AdbKey::generate().expect("device identity");
        let mut tls: SslStream<TcpStream> =
            tls_server(&identity).accept(stream).expect("tls accept");
        let peer = tls.ssl().peer_certificate().expect("client certificate");
        let peer_key = peer
            .public_key()
            .expect("client public key")
            .public_key_to_der()
            .expect("der");
        peer_key_sink.send(peer_key).expect("report key");

        Message::with_data(
            A_CNXN,
            0x0100_0001,
            u32::try_from(MAX_PAYLOAD).expect("fits"),
            b"device::\0".to_vec(),
        )
        .write_to(&mut tls)
        .expect("banner");
        let mut commands = Vec::new();
        while let Ok(message) = Message::read_from(&mut tls) {
            match message.command {
                A_OPEN => {
                    commands.push(
                        String::from_utf8_lossy(&message.data)
                            .trim_end_matches('\0')
                            .to_owned(),
                    );
                    Message::new(A_OKAY, 7, message.arg0)
                        .write_to(&mut tls)
                        .expect("okay");
                    Message::with_data(A_WRTE, 7, message.arg0, shell_output.to_vec())
                        .write_to(&mut tls)
                        .expect("data");
                }
                A_OKAY => {
                    Message::new(A_CLSE, 7, message.arg0)
                        .write_to(&mut tls)
                        .expect("close");
                    break;
                }
                _ => {}
            }
        }
        commands
    });
    (address, handle)
}

#[test]
fn shell_command_output_comes_back_over_tls() {
    let key = AdbKey::generate().expect("client key");
    let (sender, receiver) = mpsc::channel();
    let (address, server) = spawn_adbd(b"MAHIROUTE_READY 40123\r\n", sender);

    let output = run_shell(address, &key, "echo hi", Duration::from_secs(5)).expect("shell output");
    assert_eq!(output, "MAHIROUTE_READY 40123\r\n");
    assert_eq!(
        server.join().expect("server thread"),
        vec!["shell:echo hi".to_owned()]
    );

    let presented = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("peer key");
    let expected = key
        .certificate()
        .public_key()
        .expect("key")
        .public_key_to_der()
        .expect("der");
    assert_eq!(
        presented, expected,
        "the TLS client must present the app's key"
    );
}

#[test]
fn a_device_without_tls_is_reported_clearly() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let _ = Message::read_from(&mut stream).expect("request");
        Message::with_data(0x4854_5541, 1, 0, vec![0; 20])
            .write_to(&mut stream)
            .expect("auth challenge");
    });
    let key = AdbKey::generate().expect("client key");
    let error = Session::connect(address, &key).err().expect("must fail");
    assert!(
        matches!(error, AdbError::Protocol(ref text) if text.contains("wireless debugging")),
        "{error}"
    );
    server.join().expect("server thread");
}

#[test]
fn an_unreachable_device_is_an_io_error() {
    let key = AdbKey::generate().expect("client key");
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = closed.local_addr().expect("address");
    drop(closed);
    assert!(matches!(
        Session::connect(address, &key),
        Err(AdbError::Io(_))
    ));
}

#[test]
fn starting_the_helper_returns_its_port_and_a_fresh_token() {
    let key = AdbKey::generate().expect("client key");
    let (sender, _receiver) = mpsc::channel();
    let (address, server) = spawn_adbd(b"MAHIROUTE_READY 40555\r\n", sender);

    let apk = "/data/app/~~x==/io.github.mahiron_hq.mahiroute-y==/base.apk";
    let endpoint =
        mahiroute_adb::start_helper(address, &key, apk, "io.github.mahiron_hq.mahiroute")
            .expect("helper starts");
    assert_eq!(endpoint.port, 40555);
    assert_eq!(endpoint.token.len(), 32);

    let commands = server.join().expect("server thread");
    assert_eq!(commands.len(), 1);
    assert!(commands[0].starts_with(
        "shell:pm grant io.github.mahiron_hq.mahiroute android.permission.WRITE_SECURE_SETTINGS"
    ));
    assert!(commands[0].contains("kill $(pidof mahiroute-helper)"));
    assert!(commands[0].contains(&format!("--token {}", endpoint.token)));
    assert!(commands[0].contains(apk));
}

#[test]
fn a_device_that_does_not_know_the_key_is_reported_as_unpaired() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let _ = Message::read_from(&mut stream).expect("request");
        Message::new(A_STLS, 0x0100_0000, 0)
            .write_to(&mut stream)
            .expect("offer tls");
        let _ = Message::read_from(&mut stream).expect("tls reply");
        let identity = AdbKey::generate().expect("device identity");
        // The handshake itself completes; the rejection only shows up afterwards.
        let _ = tls_server_with(&identity, false).accept(stream);
    });
    let key = AdbKey::generate().expect("client key");
    let error = Session::connect(address, &key).err().expect("must fail");
    assert!(
        matches!(error, AdbError::Tls(_)),
        "an unknown key must surface as a TLS refusal: {error}"
    );
    server.join().expect("server thread");
}
