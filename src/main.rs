use std::collections::{HashMap, HashSet};
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, clap::Parser)]
struct CliArgs {
    #[arg(long, default_value = "http://127.0.0.1:8500")]
    consul_addr: String,
}

fn main() {
    let args = CliArgs::parse();

    let tracing_registry = tracing_subscriber::registry().with(tracing_subscriber::fmt::layer());
    tracing::subscriber::set_global_default(tracing_registry).unwrap();

    tracing::info!("Starting up...");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async move {
        let client = reqwest::Client::new();
        let mut running_handlers = HashMap::<
            String,
            (
                tokio::task::JoinHandle<()>,
                tokio_util::sync::CancellationToken,
            ),
        >::new();

        let consul_config = ipv_proxy::config::ConsulConfig {
            base_addr: reqwest::Url::parse(&args.consul_addr).unwrap()
        };

        loop {
            let exposed = match ipv_proxy::config::update_config(&client, &consul_config).await {
                Ok(exposed) => exposed,
                Err(e) => {
                    tracing::error!("Updating Config: {:?}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    continue;
                }
            };

            tracing::debug!(?exposed, "Exposed");

            let exposed_id_names: HashSet<String> =
                exposed.iter().map(|exp| &exp.name).cloned().collect();
            let running_id_names: HashSet<String> = running_handlers.keys().cloned().collect();

            let id_names_to_remove: HashSet<_> =
                running_id_names.difference(&exposed_id_names).collect();
            let id_names_to_start: HashSet<_> =
                exposed_id_names.difference(&running_id_names).collect();

            tracing::debug!(?id_names_to_remove, "Services to stop since last config");
            tracing::debug!(?id_names_to_start, "Services to start since last config");

            for id in id_names_to_remove {
                let (handle, cancellation) = match running_handlers.remove(id) {
                    Some(e) => e,
                    None => {
                        tracing::warn!(?id, "No running handler for id");
                        continue;
                    }
                };

                cancellation.cancel();

                // TODO
                // Should we abort the handle?
            }

            for id in id_names_to_start {
                let service = exposed.iter().find(|exp| &exp.name == id).unwrap();

                let cancel_token = tokio_util::sync::CancellationToken::new();
                let task_handle = tokio::spawn(forward(service.clone(), cancel_token.clone()));
                let value = (task_handle, cancel_token);

                running_handlers.insert(id.clone(), value);
            }

            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

#[tracing::instrument(skip(shutdown))]
async fn forward(
    service: ipv_proxy::config::ExposedService,
    shutdown: tokio_util::sync::CancellationToken,
) {
    tracing::info!("Starting Forwarder for service");

    let listener = match tokio::net::TcpListener::bind(std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        service.public_port,
    ))
    .await
    {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(?e, "Binding receiver");
            return;
        }
    };

    loop {
        tokio::select! {
            accepted_con = listener.accept() => {
                match accepted_con {
                    Ok((conn, addr)) => {
                        tokio::spawn(forward_con(conn, addr, service.target_addr, shutdown.child_token()));
                    }
                    Err(e) => {
                        tracing::warn!(?e, "Accepting connection");
                    }
                };
            }
            _ = shutdown.cancelled() => {
                    tracing::info!("Cancelling service");
                return;
            }
        };
    }
}

#[tracing::instrument(skip(client_conn, shutdown))]
async fn forward_con(
    client_conn: tokio::net::TcpStream,
    client_addr: std::net::SocketAddr,
    target_addr: std::net::SocketAddr,
    shutdown: tokio_util::sync::CancellationToken,
) {
    tracing::debug!("Starting Forwarder for Connection");

    let target_conn = match tokio::net::TcpStream::connect(target_addr).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(?e, "Connecting to target");
            return;
        }
    };

    tracing::debug!("Connected to Target");

    let (mut client_read, mut client_write) = client_conn.into_split();
    let (mut target_read, mut target_write) = target_conn.into_split();

    let mut client_read_buf: Box<[u8]> = vec![0; 1500].into_boxed_slice();
    let mut client_write_buf: Box<[u8]> = vec![0; 1500].into_boxed_slice();
    let mut target_read_buf: Box<[u8]> = vec![0; 1500].into_boxed_slice();
    let mut target_write_buf: Box<[u8]> = vec![0; 1500].into_boxed_slice();

    let mut client_write_buf_size: usize = 0;
    let mut target_write_buf_size: usize = 0;

    loop {
        tokio::select! {
                read = client_read.read(&mut client_read_buf[..(1500-target_write_buf_size)]), if target_write_buf_size < 1500 => {
            match read {
                Ok(read) if read == 0 => {
                    tracing::info!("Read EOF");
                    return ;
                }
                Ok(read) => {
                        (target_write_buf[target_write_buf_size..(target_write_buf_size+read)]).copy_from_slice(&client_read_buf[..read]);
                        target_write_buf_size += read;
                    }
                    Err(e) => {
                        tracing::error!(?e, "Read from client");
                        return;
                    }
                };
        }
                written = client_write.write(&client_write_buf[..client_write_buf_size]), if client_write_buf_size > 0 => {
                    match written {
                    Ok(written) => {
                        client_write_buf.rotate_left(written);
                    client_write_buf_size -= written;
                    }
                    Err(e) => {
                        tracing::error!(?e, "Write to client");
                        return;
                    }
                };
                }

            read = target_read.read(&mut target_read_buf[..(1500-client_write_buf_size)]), if client_write_buf_size < 1500 => {
            match read {
                Ok(read) if read == 0 => {
                    tracing::info!("Read EOF");
                    return ;
                }
                Ok(read) => {
                        (client_write_buf[client_write_buf_size..(client_write_buf_size+read)]).copy_from_slice(&target_read_buf[..read]);
                        client_write_buf_size += read;
                    }
                    Err(e) => {
                        tracing::error!(?e, "Read from Target");
                        return;
                    }
                };
        }
                written = target_write.write(&target_write_buf[..target_write_buf_size]), if target_write_buf_size > 0 => {
                    match written {
                    Ok(written) => {
                        target_write_buf.rotate_left(written);
                        target_write_buf_size -= written;
                    }
                    Err(e) => {
                        tracing::error!(?e, "Write to client");
                        return;
                    }
                };
                }
                _ = shutdown.cancelled() => {
                    return;
                }
            };
    }
}
