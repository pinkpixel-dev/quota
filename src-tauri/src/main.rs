// Release builds on Windows run as GUI apps, so they don't open a console window
// that quits Quota when it's closed. Debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    quota_lib::run();
}
