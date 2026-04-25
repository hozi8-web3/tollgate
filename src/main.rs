mod config;
mod pricing;
mod proxy;
mod agents;
mod db;
mod dashboard;
mod gui;

use anyhow::Result;
use axum::Router;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::AppConfig>,
    pub db: db::DbPool,
    pub pricing: Arc<pricing::PricingTable>,
    pub http_client: reqwest::Client,
}

/// Tollgate — LLM Cost Tracker & Optimizer
#[derive(Parser)]
#[command(name = "lct", version, about = "Tollgate — Track and optimize your LLM API costs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file
    #[arg(long, global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the proxy and dashboard servers
    Start {
        /// Don't start the dashboard server
        #[arg(long)]
        no_dashboard: bool,

        /// Launch native GUI window instead of browser
        #[arg(long)]
        gui: bool,
    },
    /// Print usage statistics
    Stats {
        /// Number of days to show
        #[arg(long, default_value = "7")]
        days: i64,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Export all logs
    Export {
        /// Output format: csv or json
        #[arg(long, default_value = "csv")]
        format: String,
        /// Output file path
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Manage configuration
    Config {
        /// Setting key
        key: String,
        /// Setting value
        value: Option<String>,
    },
    /// Update pricing data
    Pricing {
        #[command(subcommand)]
        action: PricingAction,
    },
    /// Reset (wipe) the database
    Reset,
}

#[derive(Subcommand)]
enum PricingAction {
    /// Show current pricing
    Show,
    /// Update pricing from providers (placeholder)
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lct=info,tower_http=info".parse().unwrap()),
        )
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();

    // Load config
    let cfg = config::AppConfig::load(cli.config.as_deref())?;
    let db_path = cfg.resolved_db_path();

    // Initialize database
    let db_pool = db::init(&db_path)?;

    // Load pricing
    let pricing_table = pricing::load_pricing()?;

    match cli.command {
        Commands::Start { no_dashboard, gui: use_gui } => {
            cmd_start(cfg, db_pool, pricing_table, no_dashboard, use_gui).await?;
        }
        Commands::Stats { days, json } => {
            cmd_stats(&db_pool, days, json)?;
        }
        Commands::Export { format, output } => {
            cmd_export(&db_pool, &format, output)?;
        }
        Commands::Config { key, value } => {
            cmd_config(&key, value.as_deref());
        }
        Commands::Pricing { action } => {
            cmd_pricing(action, &pricing_table);
        }
        Commands::Reset => {
            cmd_reset(&db_pool)?;
        }
    }

    Ok(())
}

/// Start the proxy (and optionally dashboard) servers.
async fn cmd_start(
    cfg: config::AppConfig,
    db_pool: db::DbPool,
    pricing_table: pricing::PricingTable,
    no_dashboard: bool,
    use_gui: bool,
) -> Result<()> {
    let state = AppState {
        config: Arc::new(cfg.clone()),
        db: db_pool,
        pricing: Arc::new(pricing_table),
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?,
    };

    // Build proxy router
    let proxy_app = Router::new()
        .route("/{provider}/{*path}", axum::routing::any(proxy::router::proxy_handler))
        .with_state(state.clone());

    let proxy_addr = format!("{}:{}", cfg.proxy.host, cfg.proxy.port);
    let proxy_listener = TcpListener::bind(&proxy_addr).await?;

    println!();
    println!("  ⚡ Tollgate — LLM Cost Tracker");
    println!("  ─────────────────────────────────");
    println!("  📡 Proxy:     http://{}", proxy_addr);

    if no_dashboard {
        println!("  📊 Dashboard: disabled");
        println!();
        tracing::info!("Proxy server starting on {}", proxy_addr);
        axum::serve(proxy_listener, proxy_app).await?;
    } else {
        let dashboard_addr = format!("{}:{}", cfg.proxy.host, cfg.dashboard.port);
        let dashboard_listener = TcpListener::bind(&dashboard_addr).await?;
        let dashboard_app = dashboard::server::create_router(state.clone());

        println!("  📊 Dashboard: http://{}", dashboard_addr);
        println!();

        // Auto-open browser
        if cfg.dashboard.auto_open && !use_gui {
            let url = format!("http://{}", dashboard_addr);
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let _ = open_browser(&url);
            });
        }

        if use_gui {
            gui::launch_gui()?;
        }

        tracing::info!("Starting proxy on {} and dashboard on {}", proxy_addr, dashboard_addr);

        // Run both servers concurrently
        tokio::select! {
            r = axum::serve(proxy_listener, proxy_app) => { r?; }
            r = axum::serve(dashboard_listener, dashboard_app) => { r?; }
        }
    }

    Ok(())
}

/// Print usage statistics to the terminal.
fn cmd_stats(db_pool: &db::DbPool, days: i64, as_json: bool) -> Result<()> {
    let stats = db::read::get_stats(db_pool, days)?;
    let models = db::read::get_model_breakdown(db_pool, days)?;

    if as_json {
        let output = serde_json::json!({
            "period_days": days,
            "spend_usd": stats.spend_usd,
            "requests": stats.requests,
            "input_tokens": stats.input_tokens,
            "output_tokens": stats.output_tokens,
            "models": models,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("  ⚡ Tollgate — {} Day Summary", days);
        println!("  ─────────────────────────────────");
        println!("  💰 Total Spend:   ${:.4}", stats.spend_usd);
        println!("  📡 Requests:      {}", stats.requests);
        println!("  🔤 Input Tokens:  {}", stats.input_tokens);
        println!("  🔤 Output Tokens: {}", stats.output_tokens);

        if stats.requests > 0 {
            println!("  📊 Avg Cost/Req:  ${:.6}", stats.spend_usd / stats.requests as f64);
        }

        if !models.is_empty() {
            println!();
            println!("  Model Breakdown:");
            for m in &models {
                println!("    {} ({}) — ${:.4} ({} reqs, {:.0}ms avg)",
                    m.model, m.provider, m.spend_usd, m.requests, m.avg_latency_ms);
            }
        }

        println!();
    }

    Ok(())
}

/// Export logs to CSV or JSON.
fn cmd_export(db_pool: &db::DbPool, format: &str, output: Option<PathBuf>) -> Result<()> {
    let rows = db::read::export_all(db_pool)?;

    let content = match format {
        "json" => serde_json::to_string_pretty(&rows)?,
        "csv" => {
            let mut wtr = csv::Writer::from_writer(Vec::new());
            wtr.write_record([
                "id", "timestamp", "provider", "model", "original_model",
                "was_substituted", "input_tokens", "output_tokens",
                "cache_read_tokens", "cache_write_tokens",
                "input_cost_usd", "output_cost_usd", "cache_read_cost_usd",
                "cache_write_cost_usd", "total_cost_usd", "latency_ms",
                "stop_reason", "task_type", "anomaly",
            ])?;
            for r in &rows {
                wtr.write_record([
                    &r.id, &r.timestamp, &r.provider, &r.model, &r.original_model,
                    &r.was_substituted.to_string(),
                    &r.input_tokens.to_string(), &r.output_tokens.to_string(),
                    &r.cache_read_tokens.to_string(), &r.cache_write_tokens.to_string(),
                    &format!("{:.6}", r.input_cost_usd), &format!("{:.6}", r.output_cost_usd),
                    &format!("{:.6}", r.cache_read_cost_usd), &format!("{:.6}", r.cache_write_cost_usd),
                    &format!("{:.6}", r.total_cost_usd), &r.latency_ms.to_string(),
                    r.stop_reason.as_deref().unwrap_or(""),
                    r.task_type.as_deref().unwrap_or(""),
                    &r.anomaly.to_string(),
                ])?;
            }
            String::from_utf8(wtr.into_inner()?)?
        }
        _ => anyhow::bail!("Unsupported format: {}. Use 'csv' or 'json'.", format),
    };

    match output {
        Some(path) => {
            std::fs::write(&path, &content)?;
            println!("Exported {} rows to {}", rows.len(), path.display());
        }
        None => {
            println!("{}", content);
        }
    }

    Ok(())
}

/// Manage configuration (print current values).
fn cmd_config(key: &str, value: Option<&str>) {
    match value {
        Some(v) => {
            println!("Config: set {} = {} (edit ~/.lct/config.toml to persist)", key, v);
        }
        None => {
            println!("Config: {} (edit ~/.lct/config.toml)", key);
        }
    }
}

/// Pricing commands.
fn cmd_pricing(action: PricingAction, table: &pricing::PricingTable) {
    match action {
        PricingAction::Show => {
            for (provider, models) in table {
                println!("\n  {} ({} models):", provider, models.len());
                for (model, p) in models {
                    println!("    {} — in: ${:.2}/1M, out: ${:.2}/1M",
                        model, p.input_per_1m, p.output_per_1m);
                }
            }
            println!();
        }
        PricingAction::Update => {
            println!("Pricing update is not yet automated. Edit pricing.json manually.");
        }
    }
}

/// Reset the database.
fn cmd_reset(db_pool: &db::DbPool) -> Result<()> {
    println!("⚠ This will delete ALL tracked data. Are you sure? (y/N)");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() == "y" {
        db::read::reset_db(db_pool)?;
        println!("✓ Database reset complete.");
    } else {
        println!("Cancelled.");
    }
    Ok(())
}

/// Open the default browser to a URL.
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", url]).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}
