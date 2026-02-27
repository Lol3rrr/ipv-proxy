use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tracing::instrument(skip(shutdown))]
pub async fn forward(
    service: crate::config::ExposedService,
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
                        tracing::debug!(?addr, "Received new connection");
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
                Ok(0) => {
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
                Ok(0) => {
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
