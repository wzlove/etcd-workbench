use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;

use log::{debug, error, info, warn};
use russh::client;
use russh::client::Handle;
use russh::keys::decode_secret_key;
use tokio::time::timeout;
use tokio::{io, select};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch};

use crate::api::settings::get_settings;
use crate::error::LogicError;
use crate::ssh::ssh_client::SshClient;
use crate::transport::connection::ConnectionSsh;

pub struct SshTunnel {
    proxy_port: u16,
    send_abort: watch::Sender<()>,
}

impl SshTunnel {
    pub async fn new(remote: ConnectionSsh, forward_host: &'static str, forward_port: u16) -> Result<Self, LogicError> {
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(10)),
            keepalive_interval: Some(Duration::from_secs(5)),
            keepalive_max: 6,
            ..<_>::default()
        };
        let config = Arc::new(config);

        let ssh_simple_info = format!("{}@{}:{}", remote.user, remote.host, remote.port);
        let client = SshClient::new(ssh_simple_info.clone());
        let addr = format!("{}:{}", remote.host, remote.port);

        let settings = get_settings().await?;

        let stream = timeout(
            Duration::from_secs(settings.ssh_connect_timeout_seconds), 
            TcpStream::connect(addr)
        )
        .await
        .map_err(|_| io::Error::new(ErrorKind::ConnectionAborted, "ssh connection timeout"))??;

        let mut session = client::connect_stream(config, stream, client).await?;

        if let Some(identity) = remote.identity {
            if let Some(key) = identity.key {
                let passphrase = if let Some(ref p) = key.passphrase {
                    Some(p.as_str())
                } else {
                    None
                };
                match decode_secret_key(String::from_utf8(key.key)?.as_str(), passphrase) {
                    Ok(key_pair) => {
                        let res = session.authenticate_publickey(remote.user, Arc::new(key_pair)).await?;
                        if !res {
                            return Err(LogicError::IoError(Error::new(ErrorKind::ConnectionAborted, "Ssh authentication failed")));
                        }
                    }
                    Err(e) => {
                        error!("decode ssh key failed: {}", e);
                        return Err(LogicError::IoError(Error::new(ErrorKind::ConnectionAborted, "Failed to parse ssh private key")));
                    }
                }
            } else if let Some(password) = identity.password {
                let res = session.authenticate_password(remote.user, password).await?;
                if !res {
                    return Err(LogicError::IoError(Error::new(ErrorKind::ConnectionAborted, "Ssh authentication failed")));
                }
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_port = listener.local_addr()?.port();

        let (send_abort, rcv_abort) = watch::channel(());

        info!("{} create ssh forward accept handler, local port is {}", ssh_simple_info, proxy_port);

        Self::handle_tcp_proxy(ssh_simple_info, listener, Arc::new(session), forward_host, forward_port, rcv_abort).await?;

        Ok(SshTunnel {
            proxy_port,
            send_abort,
        })
    }

    pub fn get_proxy_port(&self) -> u16 {
        self.proxy_port
    }

    async fn handle_tcp_proxy(
        ssh_simple_info: String,
        listener: TcpListener,
        ssh_session: Arc<Handle<SshClient>>,
        forward_host: &'static str,
        forward_port: u16,
        rcv_abort: watch::Receiver<()>,
    ) -> Result<(), LogicError> {
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let mut rcv_abort1 = rcv_abort.clone();
            let rcv_abort2 = rcv_abort.clone();

            let ssh_simple_info1 = Arc::new(ssh_simple_info);
            let ssh_simple_info2 = Arc::clone(&ssh_simple_info1);

            let accept_task = async move {
                {
                    sender.send(()).unwrap();
                }
                debug!("{} ssh accept future start", ssh_simple_info2);
                let local_port = listener.local_addr().unwrap().port();
                loop {
                    let accept_result = listener.accept().await;
                    match accept_result {
                        Ok((mut stream, addr)) => {
                            let mut rcv_abort3 = rcv_abort2.clone();
                            let ssh_session = Arc::clone(&ssh_session);
                            let ssh_simple_info3 = Arc::clone(&ssh_simple_info2);

                            debug!("ssh proxy stream task started, chain: local({}) -> local(127.0.0.1:{}) -> ssh({}) -> remote({}:{})",
                                addr, local_port, ssh_simple_info2,  forward_host, forward_port);

                            let direct_channel_result = ssh_session.channel_open_direct_tcpip(
                                forward_host,
                                forward_port as u32,
                                "127.0.0.1",
                                22,
                            ).await;

                            match direct_channel_result {
                                Ok(mut channel) => {
                                    tokio::spawn(async move {
                                        let mut channel_writer = channel.make_writer();
                                        let mut channel_reader = channel.make_reader();

                                        let (mut socket_reader, mut socket_writer) = stream.split();

                                        let proxy_task = async {
                                            loop {
                                                select! {
                                                    _ = io::copy(&mut socket_reader, &mut channel_writer) => {},
                                                    _ = io::copy(&mut channel_reader, &mut socket_writer) => {}
                                                }
                                            }
                                        };

                                        select! {
                                            _ = proxy_task => {
                                                debug!("{} ssh proxy stream task finished", ssh_simple_info3)
                                            }
                                            _abort = rcv_abort3.changed() => {
                                                debug!("{} ssh proxy stream task received abort event", ssh_simple_info3);
                                            }
                                        }
                                        debug!("{} ssh proxy stream future finished", ssh_simple_info3);
                                    });
                                }
                                Err(e) => {
                                    error!("Unable to forward messages via ssh: {e}");
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("ssh proxy listener error: {e}");
                            break;
                        }
                    }
                };
                debug!("{} ssh proxy accept loop finished", ssh_simple_info2);
            };

            select! {
                _accept = accept_task => {
                    debug!("{} ssh proxy accept task finished", ssh_simple_info1)
                }
                _abort = rcv_abort1.changed() => {
                    debug!("{} ssh proxy accept task received abort event", ssh_simple_info1);
                }
            }
            debug!("{} ssh accept future finished", ssh_simple_info1);
        });

        let _ = receiver.await?;
        Ok(())
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        match self.send_abort.send(()) {
            Ok(_) => {
                debug!("ssh send abort success")
            }
            Err(e) => {
                warn!("ssh send abort error: {e}")
            }
        }
        debug!("drop ssh tunnel");
    }
}