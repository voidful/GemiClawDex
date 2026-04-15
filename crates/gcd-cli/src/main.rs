use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use colored::Colorize;
use gcd_core::agent::PermissionLevel;
use gcd_core::config::SandboxPolicy;
use gcd_core::providers::ProviderScope;
use gcd_core::trust::TrustRuleKind;
use gcd_core::{App, AppCommand, ExecOptions};

/// GemiClawDex — Efficient Terminal AI Coding Agent
#[derive(Parser, Debug)]
#[command(name = "gcd", version, about, long_about = None)]
struct Cli {
    /// Return output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Return output as JSON Lines
    #[arg(long, global = true)]
    jsonl: bool,

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
    #[command(name = "commands")]
    Catalog {
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

        /// Permission level: suggest, auto-edit, full-auto
        #[arg(long, default_value = "auto-edit")]
        permission: String,

        /// Disable streaming output
        #[arg(long)]
        no_stream: bool,

        /// Auto-commit changes to git after execution
        #[arg(long)]
        git: bool,

        /// Enable planning mode: plan before executing
        #[arg(long)]
        plan: bool,

        /// The task instruction to execute
        #[arg(required = true)]
        task: Vec<String>,
    },

    /// Start a JSON-RPC daemon on stdio for IDE integration
    Serve,
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
    /// Replay structured events from a session
    Replay {
        id: String,
        #[arg(long)]
        turn: Option<usize>,
    },
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
        return run_repl(cli.json, cli.jsonl).await;
    }

    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let app = App::new(current_dir);
    if let Commands::Serve = cli.command.as_ref().unwrap() {
        return run_server_daemon(app).await;
    }
    let app_command = map_command(cli.command.unwrap());

    match app.handle(app_command).await {
        Ok(output) => {
            if cli.jsonl {
                println!("{}", output.render_jsonl());
            } else if cli.json {
                println!("{}", output.render_json());
            } else {
                println!("{}", output.render());
            }
        }
        Err(error) => {
            eprintln!("{}: {}", "error".red().bold(), error);
            process::exit(1);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Server Daemon — JSON-RPC over stdio
// ---------------------------------------------------------------------------

async fn run_server_daemon(app: gcd_core::App) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    
    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    // A simple loop listening to JSON inputs on stdio
    loop {
        line.clear();
        if stdin.read_line(&mut line).await? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        // Extremely simplified daemon endpoint.
        // It responds to basic requests immediately.
        // In a real Antigravity IDE, this would spawn tasks and communicate via channels.
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(input) {
            let id = json.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let method = json.get("method").and_then(|v| v.as_str()).unwrap_or("");
            
            let response = match method {
                "ping" => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": "pong"}),
                "providers/list" => {
                    match app.handle(AppCommand::ProvidersList).await {
                        Ok(out) => {
                            let val: serde_json::Value = serde_json::from_str(&out.render_json()).unwrap_or(serde_json::Value::Null);
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": val})
                        },
                        Err(e) => serde_json::json!({"jsonrpc": "2.0", "id": id, "error": e.to_string()})
                    }
                }
                _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "error": "method not found"})
            };
            
            let mut out = serde_json::to_string(&response)?;
            out.push('\n');
            stdout.write_all(out.as_bytes()).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// REPL — Interactive mode with rustyline
// ---------------------------------------------------------------------------

async fn run_repl(json: bool, jsonl: bool) -> anyhow::Result<()> {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let app = App::new(current_dir.clone());

    // Print welcome banner
    println!(
        "{}",
        "╔══════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║       GemiClawDex (GCD) Interactive Mode      ║".cyan()
    );
    println!(
        "{}",
        "║    Type your task, or /help for commands       ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════╝".cyan()
    );
    println!();

    // Show overview on start
    match app.handle(AppCommand::Overview).await {
        Ok(output) => println!("{}", output.render()),
        Err(e) => eprintln!("{}: {}", "warning".yellow(), e),
    }
    println!();

    // Setup rustyline
    let history_path = dirs_home().join(".gcd").join("history.txt");
    let mut rl = match rustyline::DefaultEditor::new() {
        Ok(editor) => editor,
        Err(_) => {
            eprintln!(
                "{}: rustyline init failed, falling back to basic input",
                "warning".yellow()
            );
            return run_repl_basic(json, jsonl).await;
        }
    };

    // Load history
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    loop {
        let readline = rl.readline(&format!("{} ", "gcd>".green().bold()));

        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(input);

                match input {
                    "/quit" | "/exit" | "/q" => {
                        println!("{}", "Goodbye!".dimmed());
                        break;
                    }
                    "/help" | "/h" => {
                        print_repl_help();
                        continue;
                    }
                    "/clear" => {
                        print!("\x1B[2J\x1B[H");
                        let _ = io::stdout().flush();
                        continue;
                    }
                    "/providers" => {
                        handle_repl_command(&app, AppCommand::ProvidersList, json, jsonl).await;
                        continue;
                    }
                    "/trust" => {
                        handle_repl_command(
                            &app,
                            AppCommand::TrustStatus { path: None },
                            json,
                            jsonl,
                        )
                        .await;
                        continue;
                    }
                    "/sessions" => {
                        handle_repl_command(&app, AppCommand::SessionsList, json, jsonl).await;
                        continue;
                    }
                    "/overview" => {
                        handle_repl_command(&app, AppCommand::Overview, json, jsonl).await;
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
                    permission: PermissionLevel::AutoEdit,
                    stream: true,
                    auto_git: false,
                    plan_only: false,
                    ide_context: None,
                };

                handle_repl_command(&app, AppCommand::Exec(exec_options), json, jsonl).await;
                println!();
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("{}", "\nInterrupted. Type /quit to exit.".dimmed());
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("{}", "\nGoodbye!".dimmed());
                break;
            }
            Err(err) => {
                eprintln!("{}: {:?}", "readline error".red(), err);
                break;
            }
        }
    }

    // Save history
    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.save_history(&history_path);

    Ok(())
}

/// Fallback REPL without rustyline (basic stdin)
async fn run_repl_basic(json: bool, jsonl: bool) -> anyhow::Result<()> {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let app = App::new(current_dir);

    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        print!("{} ", "gcd>".green().bold());
        io::stdout().flush()?;
        line.clear();

        if io::BufRead::read_line(&mut stdin.lock(), &mut line)? == 0 {
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
            _ => {}
        }

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
            permission: PermissionLevel::AutoEdit,
            stream: true,
            auto_git: false,
            plan_only: false,
            ide_context: None,
        };

        handle_repl_command(&app, AppCommand::Exec(exec_options), json, jsonl).await;
        println!();
    }

    Ok(())
}

fn print_repl_help() {
    println!("{}", "Available commands:".bold());
    println!("  {}       — Show this help", "/help, /h".cyan());
    println!("  {}  — Exit the REPL", "/quit, /exit".cyan());
    println!("  {}        — Clear the screen", "/clear".cyan());
    println!("  {}     — Show workspace overview", "/overview".cyan());
    println!("  {}   — List available providers", "/providers".cyan());
    println!("  {}        — Show trust status", "/trust".cyan());
    println!("  {}     — List saved sessions", "/sessions".cyan());
    println!();
    println!("Anything else is treated as a coding task for the agent.");
    println!();
    println!("{}", "Exec flags (use with `gcd exec`):".bold());
    println!(
        "  {} — Permission: suggest | auto-edit | full-auto",
        "--permission".cyan()
    );
    println!("  {}  — Disable streaming output", "--no-stream".cyan());
    println!("  {}         — Auto-commit changes to git", "--git".cyan());
    println!("  {}        — Enable planning mode", "--plan".cyan());
}

async fn handle_repl_command(app: &App, command: AppCommand, json: bool, jsonl: bool) {
    match app.handle(command).await {
        Ok(output) => {
            if jsonl {
                println!("{}", output.render_jsonl());
            } else if json {
                println!("{}", output.render_json());
            } else {
                println!("{}", output.render());
            }
        }
        Err(e) => eprintln!("{}: {}", "error".red(), e),
    }
}

/// Get home directory
fn dirs_home() -> PathBuf {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
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
                scope: if global {
                    ProviderScope::Global
                } else {
                    ProviderScope::Workspace
                },
            },
            ProviderCommands::Doctor { id } => AppCommand::ProvidersDoctor { id },
        },

        Commands::Catalog { cmd } => match cmd {
            CommandOps::Reload => AppCommand::CommandsReload,
        },

        Commands::Trust(sub) => match sub {
            TrustCommands::Status { path } => AppCommand::TrustStatus { path },
            TrustCommands::Set { kind, path } => {
                let rule_kind = TrustRuleKind::parse(&kind).unwrap_or_else(|| {
                    eprintln!("Invalid trust kind. Allowed: trust, untrusted, parent.");
                    process::exit(1);
                });
                let p = path
                    .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
                AppCommand::TrustSet {
                    path: p,
                    kind: rule_kind,
                }
            }
        },

        Commands::Checkpoints { cmd } => match cmd {
            CheckpointOps::List => AppCommand::CheckpointsList,
        },

        Commands::Sessions(sub) => match sub {
            SessionCommands::List => AppCommand::SessionsList,
            SessionCommands::Show { id } => AppCommand::SessionsShow { id },
            SessionCommands::Replay { id, turn } => AppCommand::SessionsReplay { id, turn },
            SessionCommands::Fork { id } => AppCommand::SessionsFork { id },
        },

        Commands::Exec {
            provider,
            sandbox,
            skill,
            checkpoint,
            resume,
            fork,
            no_session,
            no_prompt,
            permission,
            no_stream,
            git,
            plan,
            task,
        } => {
            let permission = PermissionLevel::parse(&permission).unwrap_or_else(|| {
                eprintln!("Invalid permission level. Allowed: suggest, auto-edit, full-auto.");
                process::exit(1);
            });
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
                permission,
                stream: !no_stream,
                auto_git: git,
                plan_only: plan,
                ide_context: None,
            })
        }
        Commands::Serve => unreachable!("Serve command handled in main"),
    }
}

#[cfg(test)]
mod tests {
    use super::{map_command, Commands, SessionCommands};
    use gcd_core::agent::PermissionLevel;
    use gcd_core::{AppCommand, ExecOptions};

    #[test]
    fn exec_flags_are_wired_into_exec_options() {
        let command = Commands::Exec {
            provider: Some("gemini-official".to_string()),
            sandbox: Some("read-only".to_string()),
            skill: Some("code-review".to_string()),
            checkpoint: true,
            resume: Some("session-1".to_string()),
            fork: None,
            no_session: true,
            no_prompt: true,
            permission: "full-auto".to_string(),
            no_stream: true,
            git: true,
            plan: true,
            task: vec!["review".to_string(), "src/main.rs".to_string()],
        };

        let AppCommand::Exec(ExecOptions {
            input,
            skill,
            provider,
            checkpoint,
            persist_session,
            resume_session,
            print_prompt,
            permission,
            stream,
            auto_git,
            plan_only,
            ..
        }) = map_command(command)
        else {
            panic!("expected exec command");
        };

        assert_eq!(input, "review src/main.rs");
        assert_eq!(skill.as_deref(), Some("code-review"));
        assert_eq!(provider.as_deref(), Some("gemini-official"));
        assert!(checkpoint);
        assert!(!persist_session);
        assert_eq!(resume_session.as_deref(), Some("session-1"));
        assert!(!print_prompt);
        assert_eq!(permission, PermissionLevel::FullAuto);
        assert!(!stream);
        assert!(auto_git);
        assert!(plan_only);
    }

    #[test]
    fn session_replay_maps_turn_filter() {
        let command = Commands::Sessions(SessionCommands::Replay {
            id: "session-42".to_string(),
            turn: Some(3),
        });

        let AppCommand::SessionsReplay { id, turn } = map_command(command) else {
            panic!("expected session replay command");
        };

        assert_eq!(id, "session-42");
        assert_eq!(turn, Some(3));
    }
}
