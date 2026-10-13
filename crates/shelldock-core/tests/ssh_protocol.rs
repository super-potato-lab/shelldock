use russh::{
    server::{self, Msg, Server as _, Session},
    Channel, ChannelId,
};
use shelldock_core::{
    model::{Connection, SessionEvent},
    ssh::{Credentials, OpenResult, RemoteSession},
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::mpsc};
use zeroize::Zeroizing;

#[derive(Clone)]
struct TestServer {
    width: Arc<AtomicU32>,
    channels: Arc<tokio::sync::Mutex<HashMap<ChannelId, Channel<Msg>>>>,
}
impl server::Server for TestServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        self.clone()
    }
}
impl server::Handler for TestServer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "test-user" && password == "test-password" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.lock().await.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        cols: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.width.store(cols, Ordering::Relaxed);
        session.channel_success(channel)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, b"\x1b[32mtest-ready\x1b[0m\r\n".to_vec())?;
        Ok(())
    }
    async fn window_change_request(
        &mut self,
        _: ChannelId,
        cols: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.width.store(cols, Ordering::Relaxed);
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, data.to_vec())?;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, data.to_vec())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}
fn profile(port: u16) -> Connection {
    Connection {
        id: "test".into(),
        name: "test".into(),
        host: "127.0.0.1".into(),
        port,
        username: "test-user".into(),
        group: "".into(),
        auth: "password".into(),
        private_key_path: None,
        notes: "".into(),
        has_password: false,
        protocol: "ssh".into(),
        extra: serde_json::json!({}),
    }
}
fn creds(password: &str) -> Credentials {
    Credentials {
        password: Some(Zeroizing::new(password.into())),
        passphrase: None,
    }
}
#[tokio::test]
async fn host_trust_authentication_terminal_resize_and_exec() {
    let key =
        russh::keys::PrivateKey::random(&mut rand_next::rng(), russh::keys::Algorithm::Ed25519)
            .unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(1),
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let width = Arc::new(AtomicU32::new(0));
    let server_width = width.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut server = TestServer {
            width: server_width,
            channels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        };
        server.run_on_socket(config, &listener).await.unwrap();
    });
    let p = profile(port);
    let (tx, _) = mpsc::channel(32);
    let fingerprint = match RemoteSession::open(&p, creds("test-password"), None, 80, 24, tx)
        .await
        .unwrap()
    {
        OpenResult::NeedsTrust {
            fingerprint,
            changed: false,
        } => fingerprint,
        _ => panic!("must require explicit first trust"),
    };
    let (tx, _) = mpsc::channel(32);
    assert!(RemoteSession::open(
        &p,
        creds("wrong-password"),
        Some(fingerprint.clone()),
        80,
        24,
        tx
    )
    .await
    .is_err());
    let (tx, mut rx) = mpsc::channel(32);
    let remote = match RemoteSession::open(
        &p,
        creds("test-password"),
        Some(fingerprint.clone()),
        80,
        24,
        tx,
    )
    .await
    .unwrap()
    {
        OpenResult::Connected(s) => s,
        _ => panic!("known host must connect"),
    };
    let hello = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(hello,SessionEvent::Output(data) if data.starts_with(b"\x1b[32m")));
    let unicode = "UTF-8 中文 🦀\r".as_bytes().to_vec();
    remote.input(unicode.clone()).await.unwrap();
    let echoed = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(echoed,SessionEvent::Output(data) if data==unicode));
    remote.resize(140, 40).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(width.load(Ordering::Relaxed), 140);
    assert_eq!(
        remote.exec("isolated-exec-test").await.unwrap(),
        "isolated-exec-test"
    );
    remote.close().await.unwrap();
    let (tx, _) = mpsc::channel(32);
    assert!(matches!(
        RemoteSession::open(
            &p,
            creds("test-password"),
            Some("SHA256:incorrect".into()),
            80,
            24,
            tx
        )
        .await
        .unwrap(),
        OpenResult::NeedsTrust { changed: true, .. }
    ));
    task.abort();
}
