use tracing::Instrument;

use super::ForwardingBackend;

pub struct JoolForwarding {
    jool_instance: String,
    pool6_subnet: String,
    public_ip: std::net::Ipv4Addr,
}

impl JoolForwarding {
    pub fn new(jool_instance: impl Into<String>, pool6_subnet: impl Into<String>, public_ip: std::net::Ipv4Addr) -> Self {
        Self {
            jool_instance: jool_instance.into(),
            pool6_subnet: pool6_subnet.into(),
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
    fn startup(&self) -> core::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        let instance_name = self.jool_instance.clone();
        let public_ipv4 = format!("{}", self.public_ip);
        let pool6_subnet = self.pool6_subnet.clone();

        Box::pin(async move {
            // Remove the jool instance if it exists already
            let mut command = tokio::process::Command::new("jool");
            command.args(["instance", "remove", instance_name.as_str()]);
            command.spawn().unwrap().wait().await;

            // Add the jool instance
            let mut command = tokio::process::Command::new("jool");
            command.args(["instance", "add", instance_name.as_str(), "--netfilter", "--pool6", pool6_subnet.as_str()]);
            command.spawn().unwrap().wait().await;

            // Setup pool for udp
            let mut command = tokio::process::Command::new("jool");
            command.args(["-i", instance_name.as_str(), "pool4", "add", "--udp", public_ipv4.as_str(), "1-65535"]);
            command.spawn().unwrap().wait().await;
           
            // Setup pool for tcp
            let mut command = tokio::process::Command::new("jool");
            command.args(["-i", instance_name.as_str(), "pool4", "add", "--tcp", public_ipv4.as_str(), "1-65535"]);
            command.spawn().unwrap().wait().await;
        })
    }

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
                        tracing::warn!("[Add] Command ran, but not successfully: {:?}", output);
                    }
                    Err(e) => {
                        // TODO
                        tracing::warn!("[Add] Failed to execute command: {:?}", e);
                    }
                };

                shutdown.cancelled().await;

                tracing::info!("Removing forwarding for service");

                // Remove the bib entries again
                let mut remove_command = tokio::process::Command::from(raw_remove_command);
                match remove_command.output().await {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => {
                        // TODO
                        tracing::warn!("[Remove] Command ran, but not successfully: {:?}", output);
                    }
                    Err(e) => {
                        // TODO
                        tracing::warn!("[Remove] Failed to execute command: {:?}", e);
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
            "example",
            "",
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
            "example",
            "",
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
