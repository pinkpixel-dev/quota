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
        Command::HerdrReport { force } => run_herdr_report(force).await,
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

async fn run_herdr_report(force: bool) {
    use quota_cli::herdr;

    // Check which panes need a number before touching the cache or the
    // network. A user running no Claude panes at all should never fetch,
    // and never log an error, on every agent-status event.
    let panes = match herdr::list_agent_panes() {
        Ok(panes) => panes,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    let reportable_panes: Vec<_> = panes
        .into_iter()
        .filter(|pane| herdr::provider_for_agent_kind(&pane.agent).is_some())
        .collect();

    if reportable_panes.is_empty() {
        return;
    }

    let state_dir = std::env::var("HERDR_PLUGIN_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("quota-herdr"));

    let source = format!(
        "plugin:{}",
        std::env::var("HERDR_PLUGIN_ID").unwrap_or_else(|_| "pinkpixel.quota".to_string())
    );

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0);

    let cached = herdr::read_cache(&state_dir);
    let needs_fetch = force
        || cached
            .as_ref()
            .map(|cache| cache.is_stale_at_ms(now_ms, herdr::DEFAULT_TTL_MS))
            .unwrap_or(true);

    let token = if needs_fetch {
        match fetch_local_usage().await {
            Ok(usage) => match usage.compact_token() {
                Some(token) => {
                    let cache = herdr::TokenCache {
                        fetched_at_ms: now_ms,
                        token: token.clone(),
                    };
                    if let Err(err) = herdr::write_cache(&state_dir, &cache) {
                        eprintln!("{}", err);
                    }
                    token
                }
                None => {
                    eprintln!("Claude usage returned no numbers to show");
                    std::process::exit(1);
                }
            },
            Err(err) => {
                // A failed fetch falls back to the last good value rather than
                // blanking the sidebar.
                eprintln!("{}", err);
                match cached {
                    Some(cache) => cache.token,
                    None => {
                        std::process::exit(1);
                    }
                }
            }
        }
    } else {
        match cached {
            Some(cache) => cache.token,
            None => return,
        }
    };

    let mut reported_workspaces: Vec<String> = Vec::new();

    for pane in reportable_panes {
        if let Err(err) = herdr::report_pane_token(&pane.pane_id, &source, &token) {
            eprintln!("{}", err);
        }
        if !reported_workspaces.contains(&pane.workspace_id) {
            if let Err(err) = herdr::report_workspace_token(&pane.workspace_id, &source, &token) {
                eprintln!("{}", err);
            }
            reported_workspaces.push(pane.workspace_id.clone());
        }
    }
}
