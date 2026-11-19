//! The shape of the command line.

use clap::{Arg, ArgAction, Command};

use crate::oddity::Oddity;

/// The flags that each oddity takes.
fn common(cmd: Command) -> Command {
    let cmd = cmd
        .arg(
            Arg::new("yes")
                .long("yes")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Take the default of each question, and ask nothing"),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print each step and each command, and change nothing"),
        );
    // Each oddity takes --undo, so an oddity that changes nothing gives a
    // clear message and not an error of the parser.
    cmd.arg(
        Arg::new("undo")
            .long("undo")
            .global(true)
            .action(ArgAction::SetTrue)
            .help("Revert the changes that the last runs of this oddity recorded"),
    )
}

/// The command line of `odd`, with one subcommand for each oddity.
pub fn command(oddities: &[&dyn Oddity]) -> Command {
    let mut cmd = Command::new("odd")
        .about("Niche fixes in one command, for Windows, macOS and Linux.")
        .version(crate::version::LONG)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("list")
                .about("List each oddity, with its label and its hosts")
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .help("Print JSON for a script"),
                ),
        )
        .subcommand(
            Command::new("update")
                .about("Replace odd with the latest release, after a check of its checksum")
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Check for a new release, and change nothing"),
                ),
        );
    for oddity in oddities {
        let sub = Command::new(oddity.name()).about(oddity.summary());
        let sub = common(oddity.args(sub));
        cmd = cmd.subcommand(sub);
    }
    cmd
}
