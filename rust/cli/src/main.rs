use bytes::Bytes;
use clap::{Parser, Subcommand};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    name: String,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Send(CliMessage),
    // Search(MessageFilter),
}

#[derive(Parser, Debug)]
struct CliMessage {
    #[arg(long)]
    origin_chain: String,
    #[arg(long)]
    destination_chain: String,
    #[arg(long)]
    destination_address: String,
    #[arg(long)]
    mailbox_address: String,
    #[arg(long)]
    rpc_url: String,
    #[arg(long)]
    message_bytes: String,
}

#[derive(Parser, Debug)]
struct MessageHeader {
    origin_chain: String,
    destination_chain: String,
    destination_address: String,
    mailbox_address: String,
    rpc_url: String,
}

type Message = (MessageHeader, Bytes);

fn main() {
    let args = Args::parse();

    for _ in 0..args.count {
        println!("Hello {}!", args.name)
    }
}
