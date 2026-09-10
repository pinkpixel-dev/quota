use quota_cli::args::{parse_args, Command};

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
