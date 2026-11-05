use anyhow::{Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use filespring::{Action, Engine, Request, browser};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Parser)]
#[command(version, about = "FileSpring — reviewable, verified file operations")]
struct Args {
    #[arg(long, default_value = ".filespring", global = true)]
    state: PathBuf,
    /// Explicitly authorize roots; repeat for transfers between roots.
    #[arg(long, global = true)]
    root: Vec<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, ValueEnum)]
enum Kind {
    Copy,
    Move,
}
#[derive(Subcommand)]
enum Command {
    /// List one directory; never recurse or follow links.
    List {
        path: PathBuf,
    },
    /// Save a preview only. Destination folder must already exist.
    Plan {
        #[arg(value_enum)]
        action: Kind,
        #[arg(required = true, num_args = 1..)]
        sources: Vec<PathBuf>,
        #[arg(long)]
        to: PathBuf,
    },
    Show {
        id: String,
    },
    /// Execute a saved preview. Confirmation is mandatory.
    Apply {
        id: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Explicitly reconcile and resume an interrupted plan.
    Resume {
        id: String,
        #[arg(long)]
        confirm: bool,
    },
    Undo {
        id: String,
        #[arg(long)]
        confirm: bool,
    },
    History,
    Events {
        id: String,
    },
}
fn main() -> Result<()> {
    let args = Args::parse();
    let roots = if args.root.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        args.root
    };
    let mut engine = Engine::open(&args.state, &roots)?;
    match args.command {
        Command::List { path } => {
            browser::scan(
                engine.scope(),
                &path,
                &Arc::new(AtomicBool::new(false)),
                |entries| {
                    for e in entries {
                        println!(
                            "{}\t{}\t{}{}",
                            if e.directory {
                                "DIR"
                            } else if e.link {
                                "LINK"
                            } else {
                                "FILE"
                            },
                            e.bytes,
                            e.name,
                            e.error.map(|s| format!(" [{s}]")).unwrap_or_default()
                        );
                    }
                },
            )?;
        }
        Command::Plan {
            action,
            sources,
            to,
        } => {
            let requests = sources
                .into_iter()
                .map(|source| {
                    let name = source
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("Source needs a filename"))?;
                    Ok(Request {
                        action: match action {
                            Kind::Copy => Action::Copy,
                            Kind::Move => Action::Move,
                        },
                        destination: to.join(name),
                        source,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&engine.prepare(requests)?)?
            );
        }
        Command::Show { id } => println!("{}", serde_json::to_string_pretty(&engine.load(&id)?)?),
        Command::Apply { id, confirm } | Command::Resume { id, confirm } => {
            ensure!(
                confirm,
                "Review with `show`, then pass --confirm to change files"
            );
            println!("{}", serde_json::to_string_pretty(&engine.execute(&id)?)?);
        }
        Command::Undo { id, confirm } => {
            ensure!(confirm, "Pass --confirm to undo the recorded changes");
            println!("{}", serde_json::to_string_pretty(&engine.rollback(&id)?)?);
        }
        Command::History => println!("{}", serde_json::to_string_pretty(&engine.history()?)?),
        Command::Events { id } => {
            for event in engine.events(&id)? {
                println!("{event}");
            }
        }
    }
    Ok(())
}
