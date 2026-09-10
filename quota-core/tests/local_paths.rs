use quota_core::local_paths::provider_home;

/// These tests mutate process-wide environment variables, so they run in one
/// test function rather than several. Cargo runs tests in threads, and a
/// second test clearing the same variable would race this one.
#[test]
fn resolves_provider_homes_from_the_environment_and_the_default() {
    let var = "QUOTA_TEST_PROVIDER_HOME";

    std::env::remove_var(var);
    let default_home = provider_home(var, ".example").expect("home directory");
    assert!(
        default_home.ends_with(".example"),
        "expected the default dot-directory, got {}",
        default_home.display()
    );

    std::env::set_var(var, "/tmp/relocated-provider");
    assert_eq!(
        provider_home(var, ".example").expect("override"),
        std::path::PathBuf::from("/tmp/relocated-provider")
    );

    // Shell profiles routinely leave quotes and whitespace on an exported path.
    std::env::set_var(var, "  \"/tmp/quoted-provider\"  ");
    assert_eq!(
        provider_home(var, ".example").expect("override"),
        std::path::PathBuf::from("/tmp/quoted-provider")
    );

    std::env::set_var(var, "'/tmp/single-quoted'");
    assert_eq!(
        provider_home(var, ".example").expect("override"),
        std::path::PathBuf::from("/tmp/single-quoted")
    );

    // An empty or whitespace-only override is treated as unset rather than as
    // a request to read the filesystem root.
    std::env::set_var(var, "   ");
    let fallback = provider_home(var, ".example").expect("home directory");
    assert!(
        fallback.ends_with(".example"),
        "expected the default when the override is blank, got {}",
        fallback.display()
    );

    std::env::remove_var(var);
}

#[test]
fn codex_and_grok_credential_paths_end_at_their_auth_files() {
    let codex = quota_core::codex::local::local_credentials_path().expect("codex path");
    let grok = quota_core::grok::local::local_credentials_path().expect("grok path");

    assert!(codex.ends_with("auth.json"), "got {}", codex.display());
    assert!(grok.ends_with("auth.json"), "got {}", grok.display());
    assert_ne!(codex, grok);
}
