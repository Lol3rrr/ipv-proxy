use clap::Parser;

use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, clap::Parser)]
struct CliArgs {
    #[arg(long, default_value = "http://127.0.0.1:8500")]
    consul_addr: String,
    #[arg(long, value_enum, default_value = "basic")]
    log_format: LogFormat,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
enum LogFormat {
    Basic,
    Json,
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

    runtime.block_on(ipv_proxy::manage_handlers(consul_config));
}
