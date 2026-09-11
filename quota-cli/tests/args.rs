use quota_cli::args::{
    parse_args, Command, DEFAULT_WATCH_INTERVAL_SECS, MIN_WATCH_INTERVAL_SECS,
};

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn usage_defaults_to_human_output() {
    let parsed = parse_args(&argv(&["usage"])).expect("parse");
    assert_eq!(parsed, Command::Usage { json: false });
}

#[test]
fn usage_accepts_the_json_flag() {
    let parsed = parse_args(&argv(&["usage", "--json"])).expect("parse");
    assert_eq!(parsed, Command::Usage { json: true });
}

#[test]
fn no_arguments_asks_for_help() {
    assert_eq!(parse_args(&argv(&[])), Ok(Command::Help));
}

#[test]
fn an_unknown_command_is_an_error_naming_the_command() {
    let error = parse_args(&argv(&["wat"])).expect_err("should reject");
    assert!(error.contains("wat"));
}

#[test]
fn an_unknown_flag_is_an_error_naming_the_flag() {
    let error = parse_args(&argv(&["usage", "--nope"])).expect_err("should reject");
    assert!(error.contains("--nope"));
}

#[test]
fn herdr_report_parses() {
    let parsed = parse_args(&argv(&["herdr", "report"])).expect("parse");
    assert_eq!(parsed, Command::HerdrReport { force: false });
}

#[test]
fn herdr_report_accepts_force() {
    let parsed = parse_args(&argv(&["herdr", "report", "--force"])).expect("parse");
    assert_eq!(parsed, Command::HerdrReport { force: true });
}

#[test]
fn herdr_without_a_subcommand_is_an_error() {
    assert!(parse_args(&argv(&["herdr"])).is_err());
}

#[test]
fn herdr_watch_defaults_to_the_documented_interval() {
    let parsed = parse_args(&argv(&["herdr", "watch"])).expect("parse");
    assert_eq!(
        parsed,
        Command::HerdrWatch {
            interval_secs: DEFAULT_WATCH_INTERVAL_SECS
        }
    );
}

#[test]
fn herdr_watch_accepts_an_interval() {
    let parsed = parse_args(&argv(&["herdr", "watch", "--interval", "600"])).expect("parse");
    assert_eq!(parsed, Command::HerdrWatch { interval_secs: 600 });
}

#[test]
fn the_shortest_useful_interval_is_accepted() {
    let value = MIN_WATCH_INTERVAL_SECS.to_string();
    let parsed = parse_args(&argv(&["herdr", "watch", "--interval", &value])).expect("parse");
    assert_eq!(
        parsed,
        Command::HerdrWatch {
            interval_secs: MIN_WATCH_INTERVAL_SECS
        }
    );
}

#[test]
fn an_interval_inside_the_cache_ttl_is_rejected_with_the_reason() {
    let value = (MIN_WATCH_INTERVAL_SECS - 1).to_string();
    let error =
        parse_args(&argv(&["herdr", "watch", "--interval", &value])).expect_err("should reject");
    assert!(error.contains(&MIN_WATCH_INTERVAL_SECS.to_string()));
    assert!(error.contains("cache"));
}

#[test]
fn an_interval_that_is_not_a_number_is_an_error_naming_the_value() {
    let error =
        parse_args(&argv(&["herdr", "watch", "--interval", "soon"])).expect_err("should reject");
    assert!(error.contains("soon"));
}

#[test]
fn an_interval_flag_without_a_value_is_an_error() {
    assert!(parse_args(&argv(&["herdr", "watch", "--interval"])).is_err());
}

#[test]
fn an_unknown_watch_flag_is_an_error_naming_the_flag() {
    let error = parse_args(&argv(&["herdr", "watch", "--nope"])).expect_err("should reject");
    assert!(error.contains("--nope"));
}
