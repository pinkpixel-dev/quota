use quota_cli::args::{parse_args, Command, HELP};
use quota_core::claude::local_usage::fetch_local_usage;

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    let command = match parse_args(&argv) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{}", message);
            eprintln!("\n{}", HELP);
            std::process::exit(2);
        }
    };

    match command {
        Command::Help => println!("{}", HELP),
        Command::Usage { json } => run_usage(json).await,
    }
}

async fn run_usage(json: bool) {
    match fetch_local_usage().await {
        Ok(usage) => {
            if json {
                match serde_json::to_string_pretty(&vec![&usage]) {
                    Ok(rendered) => println!("{}", rendered),
                    Err(err) => {
                        eprintln!("could not render JSON: {}", err);
                        std::process::exit(1);
                    }
                }
            } else {
                println!(
                    "claude  {}",
                    usage.compact_token().unwrap_or_else(|| "unknown".to_string())
                );
            }
        }
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    }
}
