use clap::Parser;

use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, clap::Parser)]
struct CliArgs {
    #[arg(long, default_value = "http://127.0.0.1:8500")]
    consul_addr: String,
    #[arg(long, value_enum, default_value = "basic")]
    log_format: LogFormat,
    #[arg(long, value_enum, default_value = "manual")]
    backend: Backend,

    #[arg(long)]
    public_ip: Option<std::net::Ipv4Addr>,

    #[arg(long, default_value = "ipv-proxy")]
    jool_instance_name: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
enum LogFormat {
    Basic,
    Json,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
enum Backend {
    Manual,
    Jool,
}

fn main() {
    let args = CliArgs::parse();

    let tracing_registry = tracing_subscriber::registry()
        .with((args.log_format == LogFormat::Basic).then(tracing_subscriber::fmt::layer))
        .with((args.log_format == LogFormat::Json).then(|| {
            tracing_subscriber::fmt::layer().event_format(tracing_subscriber::fmt::format::json())
        }));
    tracing::subscriber::set_global_default(tracing_registry).unwrap();

    tracing::info!("Starting up...");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let consul_config = ipv_proxy::config::ConsulConfig {
        base_addr: reqwest::Url::parse(&args.consul_addr).unwrap(),
    };

    let backend: Box<dyn ipv_proxy::forward::ForwardingBackend> = match args.backend {
        Backend::Manual => {
            tracing::info!("Using the manual backend");

            let exposed_ip = args.public_ip.unwrap_or(std::net::Ipv4Addr::new(0, 0, 0, 0));

            Box::new(ipv_proxy::forward::ManualForwarding::new(exposed_ip))
        }
        Backend::Jool => {
            tracing::info!("Using the jool backend");

            let exposed_ip = match args.public_ip {
                Some(i) => i,
                None => {
                    tracing::error!("Using the jool backend requires the configuration of the public ip");

                    panic!()
                }
            };

            Box::new(ipv_proxy::forward::JoolForwarding::new(args.jool_instance_name, exposed_ip))
        }
    };

    runtime.block_on(ipv_proxy::manage_handlers(consul_config, backend));
}
