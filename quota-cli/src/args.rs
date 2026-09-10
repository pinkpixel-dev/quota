#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Usage { json: bool },
    HerdrReport { force: bool },
    Help,
}

pub const HELP: &str = "\
quota-cli — read AI usage from locally stored agent CLI credentials

Usage:
  quota-cli usage [--json]    Print current usage
  quota-cli herdr report      Push usage tokens into the Herdr sidebar
  quota-cli --help            Show this help
";

pub fn parse_args(argv: &[String]) -> Result<Command, String> {
    let Some(first) = argv.first() else {
        return Ok(Command::Help);
    };

    match first.as_str() {
        "--help" | "-h" | "help" => Ok(Command::Help),
        "usage" => {
            let mut json = false;
            for flag in &argv[1..] {
                match flag.as_str() {
                    "--json" => json = true,
                    other => return Err(format!("unknown flag: {}", other)),
                }
            }
            Ok(Command::Usage { json })
        }
        "herdr" => match argv.get(1).map(String::as_str) {
            Some("report") => {
                let mut force = false;
                for flag in &argv[2..] {
                    match flag.as_str() {
                        "--force" => force = true,
                        other => return Err(format!("unknown flag: {}", other)),
                    }
                }
                Ok(Command::HerdrReport { force })
            }
            Some(other) => Err(format!("unknown herdr subcommand: {}", other)),
            None => Err("herdr needs a subcommand: report".to_string()),
        },
        other => Err(format!("unknown command: {}", other)),
    }
}
