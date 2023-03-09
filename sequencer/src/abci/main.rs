use application::StarknetApp;
use clap::Parser;
use tendermint_abci::ServerBuilder;
use tracing_subscriber::{filter::LevelFilter, util::SubscriberInitExt};

mod application;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Bind the TCP server to this host.
    #[clap(long, default_value = "127.0.0.1")]
    host: String,

    /// Bind the TCP server to this port.
    #[clap(short, long, default_value = "26658")]
    port: u16,

    /// The default server read buffer size, in bytes, for each incoming client
    /// connection.
    #[clap(short, long, default_value = "1048576")]
    read_buf_size: usize,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long)]
    verbose: bool,

    /// Suppress all output logging (overrides --verbose).
    #[clap(short, long)]
    quiet: bool,
}

fn main() {
    let cli: Cli = Cli::parse();
    let log_level = if cli.quiet {
        LevelFilter::OFF
    } else if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    let subscriber = tracing_subscriber::fmt()
        // Use a more compact, abbreviated log format
        .compact()
        .with_max_level(log_level)
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        // Don't display the event's target (module path)
        .with_target(false)
        // Build the subscriber
        .finish();

    subscriber.init();

    let app = StarknetApp::new();
    let server = ServerBuilder::new(cli.read_buf_size)
        .bind(format!("{}:{}", cli.host, cli.port), app)
        .unwrap();

    server.listen().unwrap();
}
