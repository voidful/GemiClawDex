use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use gemi_clawdex_core::config::SandboxPolicy;
use gemi_clawdex_core::providers::ProviderScope;
use gemi_clawdex_core::trust::TrustRuleKind;
use gemi_clawdex_core::{App, AppCommand, ExecOptions};

/// GemiClawdex — Efficient Terminal AI Coding Agent
#[derive(Parser, Debug)]
#[command(name = "gcd", version, about, long_about = None)]
struct Cli {
    /// Return output in JSON format
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show general overview of workspace and settings
    Overview,

    /// Manage language model providers
    #[command(subcommand)]
    Providers(ProviderCommands),

    /// Reload custom workspace commands and skills
    Commands {
        #[command(subcommand)]
        cmd: CommandOps,
    },

    /// Manage workspace trust boundaries
    #[command(subcommand)]
    Trust(TrustCommands),

    /// List prompt checkpoints
    Checkpoints {
        #[command(subcommand)]
        cmd: CheckpointOps,
    },

    /// Manage execution sessions
    #[command(subcommand)]
    Sessions(SessionCommands),

    /// Execute a coding task with the selected provider
    Exec {
        /// Provider ID to use for execution
        #[arg(long)]
        provider: Option<String>,

        /// Sandbox strictness (off, read-only, workspace-write, container)
        #[arg(long)]
        sandbox: Option<String>,

        /// Specific skill to enforce
        #[arg(long)]
        skill: Option<String>,

        /// Create a checkpoint before executing
        #[arg(long)]
        checkpoint: bool,

        /// Resume from a specific session ID
        #[arg(long, conflicts_with = "fork")]
        resume: Option<String>,

        /// Fork from a specific session ID
        #[arg(long)]
        fork: Option<String>,

        /// Do not persist this session
        #[arg(long)]
        no_session: bool,

        /// Do not print the constructed prompt
        #[arg(long)]
        no_prompt: bool,

        /// The task instruction to execute
        #[arg(required = true)]
        task: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ProviderCommands {
    /// List all registered and available providers
    List,
    /// View the currently active provider
    Current,
    /// Show details of a specific provider
    Show { id: String },
    /// Set the active provider
    Use {
        id: String,
        #[arg(long)]
        global: bool,
    },
    /// Run diagnostics on a provider API setup
    Doctor { id: Option<String> },
}

#[derive(Subcommand, Debug)]
enum CommandOps {
    /// Reload commands and skills configurations
    Reload,
}

#[derive(Subcommand, Debug)]
enum TrustCommands {
    /// View current trust status
    Status { path: Option<PathBuf> },
    /// Set a trust rule for a path
    Set { kind: String, path: Option<PathBuf> },
}

#[derive(Subcommand, Debug)]
enum CheckpointOps {
    /// List saved checkpoints
    List,
}

#[derive(Subcommand, Debug)]
enum SessionCommands {
    /// List historical sessions
    List,
    /// Details about a specific session
    Show { id: String },
    /// Fork a session into a new branch
    Fork { id: String },
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // If no subcommand, enter REPL mode
    if cli.command.is_none() {
        return run_repl(cli.json).await;
    }

    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let app = App::new(current_dir);
    let app_command = map_command(cli.command.unwrap());

    match app.handle(app_command) {
        Ok(output) => {
            if cli.json {
                println!("{}", output.render_json());
            } else {
                println!("{}", output.render());
            }
        }
        Err(error) => {
            eprintln!("error: {}", error);
            process::exit(1);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// REPL — Interactive mode (no subcommand given)
// ---------------------------------------------------------------------------

async fn run_repl(json: bool) -> anyhow::Result<()> {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let app = App::new(current_dir.clone());

    // Print welcome banner
    println!("╔══════════════════════════════════════════╗");
    println!("║       GemiClawdex Interactive Mode       ║");
    println!("║   Type your task, or /help for commands  ║");
    println!("╚══════════════════════════════════════════╝");
    println!();

    // Show overview on start
    match app.handle(AppCommand::Overview) {
        Ok(output) => println!("{}", output.render()),
        Err(e) => eprintln!("warning: {}", e),
    }
    println!();

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        print!("gcd> ");
        io::stdout().flush()?;
        line.clear();

        if reader.read_line(&mut line)? == 0 {
            // EOF
            println!("\nGoodbye!");
            break;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            "/quit" | "/exit" | "/q" => {
                println!("Goodbye!");
                break;
            }
            "/help" | "/h" => {
                print_repl_help();
                continue;
            }
            "/clear" => {
                print!("\x1B[2J\x1B[H");
                continue;
            }
            "/providers" => {
                handle_repl_command(&app, AppCommand::ProvidersList, json);
                continue;
            }
            "/trust" => {
                handle_repl_command(&app, AppCommand::TrustStatus { path: None }, json);
                continue;
            }
            "/sessions" => {
                handle_repl_command(&app, AppCommand::SessionsList, json);
                continue;
            }
            "/overview" => {
                handle_repl_command(&app, AppCommand::Overview, json);
                continue;
            }
            _ => {}
        }

        // Treat everything else as an exec task
        let exec_options = ExecOptions {
            input: input.to_string(),
            skill: None,
            provider: None,
            sandbox: None,
            checkpoint: false,
            persist_session: true,
            resume_session: None,
            fork_session: None,
            print_prompt: true,
        };

        handle_repl_command(&app, AppCommand::Exec(exec_options), json);
        println!();
    }

    Ok(())
}

fn print_repl_help() {
    println!("Available commands:");
    println!("  /help, /h       — Show this help");
    println!("  /quit, /exit    — Exit the REPL");
    println!("  /clear          — Clear the screen");
    println!("  /overview       — Show workspace overview");
    println!("  /providers      — List available providers");
    println!("  /trust          — Show trust status");
    println!("  /sessions       — List saved sessions");
    println!();
    println!("Anything else is treated as a coding task for the agent.");
}

fn handle_repl_command(app: &App, command: AppCommand, json: bool) {
    match app.handle(command) {
        Ok(output) => {
            if json {
                println!("{}", output.render_json());
            } else {
                println!("{}", output.render());
            }
        }
        Err(e) => eprintln!("error: {}", e),
    }
}

// ---------------------------------------------------------------------------
// Command mapping
// ---------------------------------------------------------------------------

fn map_command(cmd: Commands) -> AppCommand {
    match cmd {
        Commands::Overview => AppCommand::Overview,

        Commands::Providers(sub) => match sub {
            ProviderCommands::List => AppCommand::ProvidersList,
            ProviderCommands::Current => AppCommand::ProvidersCurrent,
            ProviderCommands::Show { id } => AppCommand::ProvidersShow { id },
            ProviderCommands::Use { id, global } => AppCommand::ProvidersUse {
                id,
                scope: if global { ProviderScope::Global } else { ProviderScope::Workspace },
            },
            ProviderCommands::Doctor { id } => AppCommand::ProvidersDoctor { id },
        },

        Commands::Commands { cmd } => match cmd {
            CommandOps::Reload => AppCommand::CommandsReload,
        },

        Commands::Trust(sub) => match sub {
            TrustCommands::Status { path } => AppCommand::TrustStatus { path },
            TrustCommands::Set { kind, path } => {
                let rule_kind = TrustRuleKind::parse(&kind).unwrap_or_else(|| {
                    eprintln!("Invalid trust kind. Allowed: trust, untrusted, parent.");
                    process::exit(1);
                });
                let p = path.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
                AppCommand::TrustSet { path: p, kind: rule_kind }
            }
        },

        Commands::Checkpoints { cmd } => match cmd {
            CheckpointOps::List => AppCommand::CheckpointsList,
        },

        Commands::Sessions(sub) => match sub {
            SessionCommands::List => AppCommand::SessionsList,
            SessionCommands::Show { id } => AppCommand::SessionsShow { id },
            SessionCommands::Fork { id } => AppCommand::SessionsFork { id },
        },

        Commands::Exec {
            provider, sandbox, skill, checkpoint, resume, fork, no_session, no_prompt, task,
        } => {
            AppCommand::Exec(ExecOptions {
                input: task.join(" "),
                skill,
                provider,
                sandbox: sandbox.as_deref().and_then(SandboxPolicy::parse),
                checkpoint,
                persist_session: !no_session,
                resume_session: resume,
                fork_session: fork,
                print_prompt: !no_prompt,
            })
        }
    }
}
