use quota_cli::args::{parse_args, Command, HELP};

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
    let reports = quota_core::providers::fetch_all().await;

    let succeeded: Vec<_> = reports
        .iter()
        .filter_map(|(_, result)| result.as_ref().ok())
        .collect();

    if json {
        // The JSON shape stays an array of usage records. A provider the user
        // is not signed into is not a record, so its message goes to stderr
        // rather than into a payload something else is parsing.
        for (name, result) in &reports {
            if let Err(message) = result {
                eprintln!("{}: {}", name, message);
            }
        }
        match serde_json::to_string_pretty(&succeeded) {
            Ok(rendered) => println!("{}", rendered),
            Err(err) => {
                eprintln!("could not render JSON: {}", err);
                std::process::exit(1);
            }
        }
    } else {
        let width = reports
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);

        // One clock reading for the whole report, so two windows that reset at
        // the same moment cannot render as different durations.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);

        for (name, result) in &reports {
            let detail = match result {
                Ok(usage) => quota_cli::render::usage_detail(usage, now)
                    .unwrap_or_else(|| "no usage numbers reported".to_string()),
                Err(message) => message.clone(),
            };
            println!("{:<width$}  {}", name, detail, width = width);
        }
    }

    // Any provider reporting is a useful run. Exiting non-zero because one
    // provider the user never signed into failed would make the command look
    // broken on a machine where it is working fine.
    if succeeded.is_empty() {
        std::process::exit(1);
    }
}

async fn run_herdr_report(force: bool) {
    use quota_cli::herdr;
    use std::collections::BTreeMap;

    // Check which panes need a number before touching the cache or the
    // network. A user running no agent we can report on should never fetch,
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
        .filter_map(|pane| {
            herdr::provider_for_agent_kind(&pane.agent).map(|provider| (pane, provider))
        })
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

    // Only the providers actually on screen. A signed-out provider with no pane
    // must not cost a request, or an error, on every status change.
    let mut needed: Vec<&'static str> = Vec::new();
    for (_, provider) in &reportable_panes {
        if !needed.contains(provider) {
            needed.push(provider);
        }
    }

    let mut tokens: BTreeMap<&'static str, String> = BTreeMap::new();
    for provider in needed {
        if let Some(token) = token_for_provider(provider, &state_dir, now_ms, force).await {
            tokens.insert(provider, token);
        }
    }

    let mut any_report_succeeded = false;

    for (pane, provider) in &reportable_panes {
        let Some(token) = tokens.get(provider) else {
            continue;
        };
        match herdr::report_pane_token(&pane.pane_id, &source, token) {
            Ok(()) => any_report_succeeded = true,
            Err(err) => eprintln!("{}", err),
        }
    }

    // The workspace row carries no agent name, so it can only be reported when
    // every reportable pane in that workspace runs the same provider. A mixed
    // workspace gets no row rather than one agent's number labelled as all.
    let mut workspace_providers: BTreeMap<&str, Option<&'static str>> = BTreeMap::new();
    for (pane, provider) in &reportable_panes {
        workspace_providers
            .entry(pane.workspace_id.as_str())
            .and_modify(|existing| {
                if *existing != Some(*provider) {
                    *existing = None;
                }
            })
            .or_insert(Some(*provider));
    }

    for (workspace_id, provider) in workspace_providers {
        let Some(token) = provider.and_then(|provider| tokens.get(provider)) else {
            continue;
        };
        match herdr::report_workspace_token(workspace_id, &source, token) {
            Ok(()) => any_report_succeeded = true,
            Err(err) => eprintln!("{}", err),
        }
    }

    if !any_report_succeeded {
        std::process::exit(1);
    }
}

/// Resolve one provider's display token, preferring a fresh cache entry.
///
/// A failed fetch falls back to the last good value rather than blanking the
/// pane, matching what the single-provider version did.
async fn token_for_provider(
    provider: &str,
    state_dir: &std::path::Path,
    now_ms: i64,
    force: bool,
) -> Option<String> {
    use quota_cli::herdr;

    let cached = herdr::read_cache(state_dir, provider);
    let needs_fetch = force
        || cached
            .as_ref()
            .map(|cache| cache.is_stale_at_ms(now_ms, herdr::DEFAULT_TTL_MS))
            .unwrap_or(true);

    if !needs_fetch {
        return cached.map(|cache| cache.token);
    }

    match quota_core::providers::fetch_provider(provider).await {
        Some(Ok(usage)) => match usage.compact_token() {
            Some(token) => {
                let cache = herdr::TokenCache {
                    fetched_at_ms: now_ms,
                    token: token.clone(),
                };
                if let Err(err) = herdr::write_cache(state_dir, provider, &cache) {
                    eprintln!("{}", err);
                }
                Some(token)
            }
            None => {
                eprintln!("{} usage returned no numbers to show", provider);
                cached.map(|cache| cache.token)
            }
        },
        Some(Err(message)) => {
            eprintln!("{}: {}", provider, message);
            cached.map(|cache| cache.token)
        }
        None => {
            eprintln!("{} has no usage reader", provider);
            None
        }
    }
}
