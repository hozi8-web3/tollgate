use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level application configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_proxy")]
    pub proxy: ProxyConfig,

    #[serde(default = "default_dashboard")]
    pub dashboard: DashboardConfig,

    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,

    #[serde(default)]
    pub routing: RoutingConfig,

    #[serde(default = "default_storage")]
    pub storage: StorageConfig,

    #[serde(default)]
    pub alerts: AlertsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProxyConfig {
    #[serde(default = "default_proxy_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DashboardConfig {
    #[serde(default = "default_dashboard_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub auto_open: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    pub api_key_env: String,
    pub base_url: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RoutingConfig {
    #[serde(default)]
    pub cost_optimize: bool,
    pub max_cost_per_request_usd: Option<f64>,
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RoutingRule {
    pub if_prompt_contains: String,
    pub use_model: String,
    pub use_provider: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AlertsConfig {
    #[serde(default = "default_anomaly_multiplier")]
    pub anomaly_multiplier: f64,
    #[serde(default = "default_daily_budget")]
    pub daily_budget_usd: f64,
}

// ── Defaults ──────────────────────────────────────────────

fn default_proxy() -> ProxyConfig {
    ProxyConfig {
        port: default_proxy_port(),
        host: default_host(),
    }
}

fn default_dashboard() -> DashboardConfig {
    DashboardConfig {
        port: default_dashboard_port(),
        auto_open: true,
    }
}

fn default_storage() -> StorageConfig {
    StorageConfig {
        db_path: default_db_path(),
        retention_days: default_retention_days(),
    }
}

fn default_proxy_port() -> u16 {
    4000
}
fn default_dashboard_port() -> u16 {
    4001
}
fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_true() -> bool {
    true
}
fn default_db_path() -> String {
    "~/.lct/usage.db".to_string()
}
fn default_retention_days() -> u32 {
    90
}
fn default_anomaly_multiplier() -> f64 {
    3.0
}
fn default_daily_budget() -> f64 {
    5.0
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            anomaly_multiplier: default_anomaly_multiplier(),
            daily_budget_usd: default_daily_budget(),
        }
    }
}

impl AppConfig {
    /// Load config from a TOML file. Falls back to defaults if file doesn't exist.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => default_config_path(),
        };

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            let config: AppConfig = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;
            Ok(config)
        } else {
            tracing::info!("No config file found at {}, using defaults", config_path.display());
            Ok(AppConfig::default())
        }
    }

    /// Resolve the database path (expand ~ to home directory).
    pub fn resolved_db_path(&self) -> PathBuf {
        expand_tilde(&self.storage.db_path)
    }

    /// Get the API key for a provider by reading the environment variable.
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        self.providers
            .get(provider)
            .and_then(|p| std::env::var(&p.api_key_env).ok())
    }

    /// Get the base URL for a provider.
    pub fn get_base_url(&self, provider: &str) -> Option<String> {
        self.providers.get(provider).map(|p| p.base_url.clone())
    }

    /// Get list of allowed/configured provider names.
    pub fn allowed_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "anthropic".to_string(),
            ProviderConfig {
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
            },
        );
        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                api_key_env: "OPENAI_API_KEY".to_string(),
                base_url: "https://api.openai.com".to_string(),
            },
        );
        providers.insert(
            "groq".to_string(),
            ProviderConfig {
                api_key_env: "GROQ_API_KEY".to_string(),
                base_url: "https://api.groq.com/openai".to_string(),
            },
        );

        Self {
            proxy: default_proxy(),
            dashboard: default_dashboard(),
            providers,
            routing: RoutingConfig::default(),
            storage: default_storage(),
            alerts: AlertsConfig::default(),
        }
    }
}

/// Get the default config file path: ~/.lct/config.toml
pub fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lct")
        .join("config.toml")
}

/// Expand ~ to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") || path.starts_with("~\\") {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(&path[2..])
    } else if path == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(path)
    }
}
