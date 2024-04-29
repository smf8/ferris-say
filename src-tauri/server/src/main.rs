use clap::Parser;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    port: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_default())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tokio::spawn(async move {
        let _server = websocket::server::server_init(&args.port).await;
    });

    tokio::signal::ctrl_c().await.unwrap();

    Ok(())
}
