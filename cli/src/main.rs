use std::{path::PathBuf, str::FromStr};

use clap::{arg, Command};

mod run;

fn cli() -> Command<'static> {
    Command::new("hbw")
        .about("Run stuff locally")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .allow_invalid_utf8_for_external_subcommands(true)
        .subcommand(
            Command::new("run")
                .about("Runs script from this directory")
                .arg(arg!(<PATH> "The path to the script"))
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

            run::start(path).await;
        }
        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachabe!()
    }
}
