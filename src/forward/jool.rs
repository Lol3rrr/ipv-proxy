use tracing::Instrument;

use super::ForwardingBackend;

pub struct JoolForwarding {
    jool_instance: String,
    public_ip: std::net::Ipv4Addr,
}

impl JoolForwarding {
    pub fn new(jool_instance: String, public_ip: std::net::Ipv4Addr) -> Self {
        Self {
            jool_instance,
            public_ip,
        }
    }

    fn construct_add_command(
        &self,
        port: u16,
        target: std::net::SocketAddrV6,
        protocol: crate::config::ServiceProtocol,
    ) -> std::process::Command {
        let mut command = std::process::Command::new("jool");

        let protocol_arg = match protocol {
            crate::config::ServiceProtocol::TCP => "--tcp",
            crate::config::ServiceProtocol::UDP => "--udp",
        };

        command.args([
            "-i",
            &format!("\"{}\"", self.jool_instance),
            "bib",
            "add",
            &format!("{}#{}", target.ip(), target.port()),
            &format!("{}#{}", self.public_ip, port),
            protocol_arg,
        ]);

        command
    }

    fn construct_remove_command(
        &self,
        port: u16,
        target: std::net::SocketAddrV6,
        protocol: crate::config::ServiceProtocol,
    ) -> std::process::Command {
        let mut command = std::process::Command::new("jool");

        let protocol_arg = match protocol {
            crate::config::ServiceProtocol::TCP => "--tcp",
            crate::config::ServiceProtocol::UDP => "--udp",
        };

        command.args([
            "-i",
            &format!("\"{}\"", self.jool_instance),
            "bib",
            "remove",
            &format!("{}#{}", target.ip(), target.port()),
            &format!("{}#{}", self.public_ip, port),
            protocol_arg,
        ]);

        command
    }
}

impl ForwardingBackend for JoolForwarding {
    fn forward(
        &self,
        service: crate::config::ExposedService,
        shutdown: tokio_util::sync::CancellationToken,
    ) -> core::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        let target = match service.target_addr {
            std::net::SocketAddr::V4(_) => todo!(),
            std::net::SocketAddr::V6(addr) => addr,
        };

        let raw_add_command =
            self.construct_add_command(service.public_port, target, service.protocol);
        let raw_remove_command =
            self.construct_remove_command(service.public_port, target, service.protocol);

        Box::pin(
            async move {
                tracing::info!("Configuring forwarding for service");

                // Add the bib entries
                let mut add_command = tokio::process::Command::from(raw_add_command);
                match add_command.output().await {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => {
                        // TODO
                    }
                    Err(e) => {
                        // TODO
                    }
                };

                shutdown.cancelled().await;

                // Remove the bib entries again
                let mut remove_command = tokio::process::Command::from(raw_remove_command);
                match remove_command.output().await {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => {
                        // TODO
                    }
                    Err(e) => {
                        // TODO
                    }
                };
            }
            .instrument(tracing::span!(
                tracing::Level::INFO,
                "JoolForwarding",
                ?service
            )),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_tcp_command() {
        let jool_instance = JoolForwarding::new(
            "example".to_string(),
            std::net::Ipv4Addr::new(51, 158, 177, 228),
        );
        let command = jool_instance.construct_add_command(
            30033,
            std::net::SocketAddrV6::new(
                std::net::Ipv6Addr::from_segments([
                    0x2001, 0x4dd5, 0xb276, 0x1, 0xf652, 0x14ff, 0xfe94, 0xdc00,
                ]),
                30033,
                0,
                0,
            ),
            crate::config::ServiceProtocol::TCP,
        );
        dbg!(&command);
    }

    #[test]
    fn remove_tcp_command() {
        let jool_instance = JoolForwarding::new(
            "example".to_string(),
            std::net::Ipv4Addr::new(51, 158, 177, 228),
        );
        let command = jool_instance.construct_remove_command(
            30033,
            std::net::SocketAddrV6::new(
                std::net::Ipv6Addr::from_segments([
                    0x2001, 0x4dd5, 0xb276, 0x1, 0xf652, 0x14ff, 0xfe94, 0xdc00,
                ]),
                30033,
                0,
                0,
            ),
            crate::config::ServiceProtocol::TCP,
        );
        dbg!(&command);
    }
}
