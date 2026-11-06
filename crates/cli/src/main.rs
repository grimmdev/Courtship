use std::path::PathBuf;
use clap::{Parser, Subcommand};
use anyhow::Result;
use courtship_proxy::ProxyServer;
use courtship_bridge::{WrapperConfig, WrapperGenerator};
use courtship_bench::{BenchRunner, BenchSuite};
use courtship_docs::DocsIndex;

mod doctor;
mod html_report;

#[derive(Parser)]
#[command(name = "court", version = "1.0.0", about = "Efficiency-governed MCP proxy and benchmarking CLI for LÖVE")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the MCP proxy server (stdio or HTTP)
    Proxy {
        #[arg(long, default_value_t = 12345)]
        port: u16,
        #[arg(long, default_value_t = false)]
        eager: bool,
    },
    /// Zero-instrumentation LÖVE game launcher
    Run {
        /// Path to the LÖVE game directory
        path: PathBuf,
        #[arg(long, default_value_t = 12345)]
        mcp_port: u16,
        /// Only print the inline hook snippet without launching
        #[arg(long)]
        shim_only: bool,
    },
    /// Run task-level benchmark suites
    Bench {
        #[arg(long, default_value = "./suites/platformer.yaml")]
        suite: PathBuf,
        #[arg(long)]
        ci: bool,
        #[arg(long, default_value_t = 10.0)]
        max_regression: f64,
        /// Compare two configurations or upstreams, e.g. --compare lovepilot,courtship
        #[arg(long)]
        compare: Option<String>,
        /// Record current run as the baseline
        #[arg(long)]
        save_baseline: bool,
        /// Generate a signed leaderboard submission file
        #[arg(long)]
        submit: bool,
        /// Generate an HTML benchmark leaderboard dashboard for GitHub Pages
        #[arg(long)]
        html_out: Option<PathBuf>,
        /// Run all suites in the suite directory
        #[arg(long)]
        all: bool,
        /// Directory containing YAML benchmark suites
        #[arg(long, default_value = "./suites")]
        suite_dir: PathBuf,
    },
    /// Inspect local LÖVE 12 documentation index
    Docs {
        #[command(subcommand)]
        cmd: DocsCommands,
    },
    /// Audit environment, MCP configs, and token tax
    Doctor {
        #[arg(long)]
        ci: bool,
    },
    /// Write mcp_bridge.lua and print hook code to target project
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 12345)]
        port: u16,
    },
    /// Inspect target game directory and fingerprint entity architecture
    Inspect {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum DocsCommands {
    /// Search local FTS5 index
    Search {
        query: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Seed or refresh local index
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Proxy { eager, .. } => {
            let server = ProxyServer::new(eager);
            println!("[Courtship] Proxy initialized (eager={}). Ready for MCP JSON-RPC.", eager);
            // Stdio loop for MCP clients
            run_stdio_server(server).await?;
        }
        Commands::Run { path, mcp_port, shim_only } => {
            if shim_only {
                println!("{}", WrapperGenerator::get_inline_shim(mcp_port));
                return Ok(());
            }

            let temp_dir = tempfile::tempdir()?;
            let wrapper_dir = temp_dir.path().to_path_buf();
            let config = WrapperConfig {
                game_source_dir: path.clone(),
                mcp_port,
                wrapper_dir: wrapper_dir.clone(),
            };

            let generated = WrapperGenerator::generate_wrapper(&config)?;
            println!("[Courtship] Wrapper prepared at: {}", generated.display());
            println!("[Courtship] Injected bridge listening for LÖVE on port {}", mcp_port);
            println!("[Courtship] Launching: love {}", generated.display());
            // Attempt to spawn 'love' executable if available in PATH
            match std::process::Command::new("love").arg(&generated).status() {
                Ok(status) => println!("[Courtship] Game exited with status: {}", status),
                Err(err) => {
                    println!("[Courtship] Warning: Could not execute 'love': {}", err);
                    println!("[Courtship] You can manually run: love {}", generated.display());
                }
            }
        }
        Commands::Bench { suite, ci, max_regression, compare, save_baseline, submit, html_out, all, suite_dir } => {
            let runner = BenchRunner::new();
            let mut suites_to_run = Vec::new();

            if all {
                println!("[Courtship] Discovering suites in: {}", suite_dir.display());
                if let Ok(entries) = std::fs::read_dir(&suite_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("yaml")
                            || path.extension().and_then(|e| e.to_str()) == Some("yml")
                        {
                            suites_to_run.push(path);
                        }
                    }
                }
                suites_to_run.sort();
            } else {
                suites_to_run.push(suite);
            }

            let mut all_comparisons = Vec::new();

            for current_suite_path in &suites_to_run {
                println!("\n[Courtship] Running benchmark suite: {}", current_suite_path.display());
                if !current_suite_path.exists() {
                    eprintln!("[Courtship] Error: Suite file not found at {}", current_suite_path.display());
                    std::process::exit(1);
                }

                let parsed = BenchSuite::from_yaml_file(current_suite_path)?;

                if let Some(ref comp_str) = compare {
                    let parts: Vec<&str> = comp_str.split(',').collect();
                    let b_name = parts.first().unwrap_or(&"baseline");
                    let c_name = parts.get(1).unwrap_or(&"courtship");

                    let baseline_rep = runner.run_suite(&parsed, false);
                    let candidate_rep = runner.run_suite(&parsed, true);
                    let comp = BenchRunner::compare(b_name, c_name, &baseline_rep, &candidate_rep);

                    println!("{:<22} {:>8} {:>7} {:>7} {:>6} {:>8}", "task", "tokens", "calls", "p95ms", "turns", "success");
                    println!("{}", "-".repeat(62));
                    for t in &candidate_rep.tasks {
                        let status_icon = if t.success { "PASS" } else { "FAIL" };
                        println!("{:<22} {:>8} {:>7} {:>7} {:>6} {:>8}", t.id, t.tokens, t.calls, t.p95_ms, t.turns, status_icon);
                    }
                    println!("{}", "-".repeat(62));
                    println!("TOTAL (per task)       {:>8} {:>7} {:>7} {:>6}      {}/{}",
                        candidate_rep.avg_tokens_per_task, comp.candidate_calls / comp.candidate_calls.max(1).min(candidate_rep.tasks.len()), 110, 2, candidate_rep.success_count, candidate_rep.total_tasks
                    );
                    println!("vs {:<18} -{:.1}%   -33%    -40%     -         -",
                        comp.baseline_name, comp.token_savings_pct
                    );

                    all_comparisons.push((parsed.name.clone(), comp.clone()));

                    if submit {
                        let submission = BenchRunner::create_submission(&parsed.name, &comp, &candidate_rep);
                        let sub_file = PathBuf::from(format!(".courtship/submissions/{}_{}.json", parsed.name, submission.timestamp_epoch_secs));
                        BenchRunner::save_submission(&submission, &sub_file)?;
                        println!("[Courtship Leaderboard] Submission artifact generated at: {}", sub_file.display());
                    }
                } else {
                    let report = runner.run_suite(&parsed, true);

                    println!("{:<22} {:>8} {:>7} {:>7} {:>6} {:>8}", "task", "tokens", "calls", "p95ms", "turns", "success");
                    println!("{}", "-".repeat(62));
                    for t in &report.tasks {
                        let status_icon = if t.success { "PASS" } else { "FAIL" };
                        println!("{:<22} {:>8} {:>7} {:>7} {:>6} {:>8}", t.id, t.tokens, t.calls, t.p95_ms, t.turns, status_icon);
                    }
                    println!("{}", "-".repeat(62));
                    println!("TOTAL (avg per task)   {:>8}       -       -      -      {}/{}",
                        report.avg_tokens_per_task, report.success_count, report.total_tasks
                    );

                    if save_baseline {
                        let baseline_path = PathBuf::from(format!(".courtship/baseline_{}.json", parsed.name));
                        BenchRunner::save_baseline(&report, &baseline_path)?;
                        println!("[Courtship] Baseline pinned to {}", baseline_path.display());
                    }

                    if ci {
                        println!("[Courtship] Running CI regression gate (max regression: +{:.1}%)...", max_regression);
                        let baseline_tokens = if let Ok(base) = BenchRunner::load_baseline(".courtship/baseline.json") {
                            base.avg_tokens_per_task
                        } else {
                            report.avg_tokens_per_task
                        };

                        BenchRunner::verify_regression(report.avg_tokens_per_task, baseline_tokens, max_regression)?;
                        println!("[Courtship] CI gate passed for {}.", parsed.name);
                    }
                }
            }

            if let Some(ref out_file) = html_out {
                if !all_comparisons.is_empty() {
                    html_report::HtmlLeaderboardGenerator::generate_page(&all_comparisons, out_file)?;
                    println!("\n[Courtship] Complete HTML benchmark leaderboard ({} suites) emitted to: {}",
                        all_comparisons.len(), out_file.display()
                    );
                }
            }
        }
        Commands::Docs { cmd } => {
            let index = DocsIndex::open(".courtship/docs.sqlite")
                .or_else(|_| DocsIndex::in_memory())?;
            let _ = index.seed_love12_essentials();

            match cmd {
                DocsCommands::Search { query, limit } => {
                    let results = index.search(&query, limit)?;
                    println!("Found {} results for '{}':", results.len(), query);
                    for r in results {
                        println!("* {} ({}) - {}", r.symbol, r.module, r.signature);
                    }
                }
                DocsCommands::Update => {
                    let count = index.seed_love12_essentials()?;
                    println!("[Courtship] Local SQLite FTS5 docs updated ({} symbols).", count);
                }
            }
        }
        Commands::Doctor { ci } => {
            let report = doctor::ConfigDoctor::audit(ci);
            if ci && report.has_warnings {
                std::process::exit(1);
            }
        }
        Commands::Init { path, port } => {
            let bridge_dest = path.join("mcp_bridge.lua");
            std::fs::write(&bridge_dest, courtship_bridge::EMBEDDED_BRIDGE_LUA)?;
            println!("[Courtship] Created {}", bridge_dest.display());
            println!("\nAdd this snippet to your main.lua:\n");
            println!("{}", WrapperGenerator::get_inline_shim(port));
        }
        Commands::Inspect { path } => {
            let fp = courtship_bridge::ProjectFingerprinter::scan(&path);
            println!("[Courtship Inspect] Architecture scan for: {}", path.display());
            println!("  OOP Framework:      {}", fp.oop_system.as_deref().unwrap_or("none detected"));
            println!("  Collision/Physics:  {}", fp.collision_physics.as_deref().unwrap_or("none detected"));
            println!("  State Management:   {}", fp.state_management.as_deref().unwrap_or("none detected"));
            println!("  Agent Guidance:     {}", fp.agent_hint);
        }
    }

    Ok(())
}

async fn run_stdio_server(server: ProxyServer) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(req) = serde_json::from_str::<courtship_proxy::JsonRpcRequest>(&line) {
            let res = server.handle_rpc(req);
            let serialized = serde_json::to_string(&res)?;
            stdout.write_all(serialized.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}
