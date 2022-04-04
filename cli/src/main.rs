use std::{
    path::PathBuf,
    str::FromStr,
    time::{Duration, Instant},
};

use clap::{arg, Command};
use notify::{watcher, RecursiveMode, Watcher};
use tokio::task::JoinHandle;

mod run;

fn cli() -> Command<'static> {
    Command::new("hbw")
        .about("Run homebrew-workers locally")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .allow_invalid_utf8_for_external_subcommands(true)
        .subcommand(
            Command::new("run")
                .about("Runs script from this directory")
                .arg(arg!(<PATH> "The path to the script"))
                .arg(arg!(-w --watch "Watch the directory"))
                .arg_required_else_help(true),
        )
}

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("run", sub_matches)) => {
            let path_str = sub_matches.value_of("PATH").expect("required");
            let path = PathBuf::from_str(path_str).expect("Failed to build pathbuf");

            let is_watch_mode = sub_matches.is_present("watch");
            if is_watch_mode {
                let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();
                let (server_tx, mut server_rx) = tokio::sync::mpsc::channel::<()>(1);

                let mut watcher = watcher(watcher_tx, Duration::from_millis(400)).unwrap();
                watcher
                    .watch(path.clone(), RecursiveMode::Recursive)
                    .unwrap();

                tokio::spawn(async move {
                    let mut current_handle: Option<JoinHandle<()>> = None;
                    while let Some(_) = server_rx.recv().await {
                        // stop server from previous loop
                        if let Some(handle) = current_handle {
                            println!("Restarting server...");
                            handle.abort();
                        } else {
                            println!("Starting server...");
                        }

                        let path = path.clone();
                        let handle = tokio::spawn(async move {
                            run::start(path).await;
                        });
                        current_handle = Some(handle);
                    }
                });

                // start server initially, otherwise no server is ever started
                server_tx.send(()).await.unwrap();
                let mut last_restart = Instant::now();

                // signal to server to restart
                for _ in watcher_rx.iter() {
                    // only restart if theres 500 millis between last restart
                    if last_restart.elapsed().as_millis() < 500 {
                        continue;
                    }
                    server_tx.send(()).await.unwrap();
                    last_restart = Instant::now();
                }
            } else {
                run::start(path).await;
            }
        }
        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachabe!()
    }
}
