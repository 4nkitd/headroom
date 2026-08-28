//! Headroom: macOS Menu Bar AI Subscription Usage Tracker in Rust + GPUI.

mod app_state;
mod assets;
mod autostart;
mod credentials;
mod diagnostics;
mod model;
mod providers;
mod settings;
mod theme;
mod ui;
mod update;

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
        println!(
            "Headroom v{} — macOS Menu Bar AI Subscription Usage Tracker",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("USAGE:");
        println!("    headroom [FLAGS] [COMMAND]");
        println!();
        println!("COMMANDS:");
        println!("    enable           Register Headroom to start automatically at login");
        println!("    disable          Unregister Headroom from starting at login");
        println!();
        println!("FLAGS:");
        println!("    -d, --detach     Run Headroom in the background (daemonize)");
        println!("    -v, --version    Print version information");
        println!("    -h, --help       Print help information");
        return;
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("headroom {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args
        .iter()
        .any(|a| a == "diagnostics" || a == "--diagnostics")
    {
        match diagnostics::static_report_json() {
            Ok(report) => println!("{report}"),
            Err(error) => {
                eprintln!("Failed to create diagnostics: {error:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Enable command
    if args
        .iter()
        .any(|a| a == "enable" || a == "--enable" || a == "autostart")
    {
        match autostart::enable() {
            Ok(_) => {
                println!("Headroom enabled to start automatically at login.");
                return;
            }
            Err(err) => {
                eprintln!("Failed to enable auto-start at login: {err}");
                std::process::exit(1);
            }
        }
    }

    // Disable command
    if args.iter().any(|a| a == "disable" || a == "--disable") {
        match autostart::disable() {
            Ok(_) => {
                println!("Headroom disabled from starting automatically at login.");
                return;
            }
            Err(err) => {
                eprintln!("Failed to disable auto-start at login: {err}");
                std::process::exit(1);
            }
        }
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

    let app = Application::new().with_assets(assets::EmbeddedAssets);

    app.run(move |cx: &mut App| {
        ui::text_input::bind_keys(cx);
        ui::bind_keys(cx);
        cx.set_global(ui::Fonts::resolve(cx));
        let state = cx.new(AppState::new);

        // No window at startup — the popover opens on demand from the menu bar.
        #[cfg(target_os = "macos")]
        status_item::setup_status_bar_item(cx, state);
    });
}
