//! Verifies against a real child process that an AppImage environment does not
//! leak into whatever the app launches to open a link.
//!
//! This runs in its own test binary because it edits the process environment,
//! which every thread in a binary shares.

#![cfg(target_os = "linux")]

use std::process::Command;

use quota_lib::external_open::sanitize_appimage_env;

/// Reads the environment a child actually receives.
fn child_env(command: &mut Command) -> Vec<(String, String)> {
    let output = command.output().expect("failed to run `env`");
    assert!(output.status.success(), "`env` did not succeed");

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

fn value_of<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Every case lives in one test because they edit the same process environment,
/// and test functions in a binary run in parallel threads.
#[test]
fn an_appimage_never_leaks_its_environment_into_the_browser_launcher() {
    the_appimage_environment_is_removed_only_when_running_from_an_appimage();
    the_launcher_receives_the_url_with_a_clean_environment_and_falls_back_on_failure();
}

fn the_appimage_environment_is_removed_only_when_running_from_an_appimage() {
    let appdir = "/tmp/.mount_quotaTEST";

    std::env::set_var("APPDIR", appdir);
    std::env::set_var("APPIMAGE", "/home/user/Quota.AppImage");
    std::env::set_var(
        "LD_LIBRARY_PATH",
        format!("{appdir}/usr/lib:{appdir}/usr/lib64"),
    );
    std::env::set_var("GIO_MODULE_DIR", "/__quota_appimage_disabled_gio_modules__");
    std::env::set_var("GTK_THEME", "Adwaita:dark");
    std::env::set_var("GDK_BACKEND", "x11");
    std::env::set_var("PATH", format!("{appdir}/usr/bin:/usr/local/bin:/usr/bin"));
    std::env::set_var(
        "XDG_DATA_DIRS",
        format!("{appdir}/usr/share:/usr/share:/usr/local/share"),
    );

    let mut polluted = Command::new("env");
    let polluted = child_env(&mut polluted);
    assert!(
        value_of(&polluted, "LD_LIBRARY_PATH").is_some(),
        "the test setup should reproduce a polluted environment"
    );

    let mut cleaned_command = Command::new("env");
    sanitize_appimage_env(&mut cleaned_command);
    let cleaned = child_env(&mut cleaned_command);

    for name in [
        "LD_LIBRARY_PATH",
        "GIO_MODULE_DIR",
        "GTK_THEME",
        "GDK_BACKEND",
    ] {
        assert_eq!(
            value_of(&cleaned, name),
            None,
            "{name} still reached the child process"
        );
    }

    assert_eq!(
        value_of(&cleaned, "PATH"),
        Some("/usr/local/bin:/usr/bin"),
        "host PATH entries must survive while AppDir entries are dropped"
    );
    assert_eq!(
        value_of(&cleaned, "XDG_DATA_DIRS"),
        Some("/usr/share:/usr/local/share"),
        "host data dirs must survive while AppDir entries are dropped"
    );

    // A normal deb/rpm install has no AppDir, so nothing should be touched.
    std::env::remove_var("APPDIR");
    std::env::set_var("LD_LIBRARY_PATH", "/opt/vendor/lib");

    let mut command = Command::new("env");
    sanitize_appimage_env(&mut command);

    assert_eq!(
        value_of(&child_env(&mut command), "LD_LIBRARY_PATH"),
        Some("/opt/vendor/lib"),
        "a normal install must keep its own library path"
    );
}

/// Writes an executable stub that records its arguments and environment.
fn write_stub_launcher(dir: &std::path::Path, name: &str, exit_code: i32) -> std::path::PathBuf {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let log = dir.join(format!("{name}.log"));
    let script = dir.join(name);

    let mut file = std::fs::File::create(&script).expect("failed to create stub launcher");
    writeln!(file, "#!/bin/sh").unwrap();
    writeln!(file, "echo \"args:$*\" >> \"{}\"", log.display()).unwrap();
    writeln!(
        file,
        "echo \"ld:${{LD_LIBRARY_PATH-unset}}\" >> \"{}\"",
        log.display()
    )
    .unwrap();
    writeln!(file, "exit {exit_code}").unwrap();
    drop(file);

    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("failed to mark stub launcher executable");

    log
}

fn the_launcher_receives_the_url_with_a_clean_environment_and_falls_back_on_failure() {
    let dir = std::env::temp_dir().join(format!("quota-launcher-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create the test directory");

    // `xdg-open` fails so the fallback to `gio` is exercised too.
    let xdg_log = write_stub_launcher(&dir, "xdg-open", 1);
    let gio_log = write_stub_launcher(&dir, "gio", 0);

    let appdir = "/tmp/.mount_quotaTEST";
    let host_path = std::env::var("PATH").unwrap_or_default();

    std::env::set_var("APPDIR", appdir);
    std::env::set_var("LD_LIBRARY_PATH", format!("{appdir}/usr/lib"));
    std::env::set_var(
        "PATH",
        format!("{}:{appdir}/usr/bin:{host_path}", dir.display()),
    );

    let url = "https://claude.com/cai/oauth/authorize?state=test";
    quota_lib::external_open::spawn_launchers(url).expect("a launcher should have handled the URL");

    let xdg_output = std::fs::read_to_string(&xdg_log).expect("xdg-open stub did not run");
    let gio_output = std::fs::read_to_string(&gio_log).expect("gio stub did not run");

    assert!(
        xdg_output.contains(&format!("args:{url}")),
        "xdg-open should be tried first with the URL, got: {xdg_output}"
    );
    assert!(
        gio_output.contains(&format!("args:open {url}")),
        "gio should be the fallback, got: {gio_output}"
    );

    for (name, output) in [("xdg-open", &xdg_output), ("gio", &gio_output)] {
        assert!(
            output.contains("ld:unset"),
            "{name} inherited LD_LIBRARY_PATH, which is what breaks the browser: {output}"
        );
    }

    std::env::set_var("PATH", host_path);
    std::fs::remove_dir_all(&dir).ok();
}
