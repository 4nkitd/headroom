//! Headroom: macOS Menu Bar AI Subscription Usage Tracker in Rust + GPUI.

mod app_state;
mod credentials;
mod model;
mod providers;
mod theme;
mod ui;

#[cfg(target_os = "macos")]
mod status_item;

use std::env;
use std::process::{Command, Stdio};

use app_state::AppState;
use gpui::{App, AppContext, Application};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Help / Version flags
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Headroom v0.1.0 — macOS Menu Bar AI Subscription Usage Tracker");
        println!();
        println!("USAGE:");
        println!("    headroom [FLAGS]");
        println!();
        println!("FLAGS:");
        println!("    -d, --detach     Run Headroom in the background (daemonize)");
        println!("    -v, --version    Print version information");
        println!("    -h, --help       Print help information");
        return;
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("headroom 0.1.0");
        return;
    }

    // Detach flag (-d / --detach / --daemon)
    if args
        .iter()
        .any(|a| a == "-d" || a == "--detach" || a == "--daemon")
    {
        let exe = env::current_exe().expect("failed to resolve executable path");
        let child_args: Vec<String> = args
            .into_iter()
            .skip(1)
            .filter(|a| a != "-d" && a != "--detach" && a != "--daemon")
            .collect();

        match Command::new(&exe)
            .args(&child_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                println!("Headroom started in background (PID {}).", child.id());
                return;
            }
            Err(err) => {
                eprintln!("Failed to start background process: {err}");
                std::process::exit(1);
            }
        }
    }

    let app = Application::new();

    app.run(move |cx: &mut App| {
        ui::text_input::bind_keys(cx);
        let state = cx.new(|cx| AppState::new(cx));
        AppState::set_global(state.clone(), cx);

        // No window at startup — the popover opens on demand from the menu bar.
        #[cfg(target_os = "macos")]
        status_item::setup_status_bar_item(cx, state);
    });
}
