# ⚡ Tollgate — LLM Cost Tracker

> Track, analyze, and optimize your LLM API costs. Single binary. Zero config. Works with any SDK.

[![CI](https://github.com/hozi8-web3/tollgate/actions/workflows/ci.yml/badge.svg)](https://github.com/hozi8-web3/tollgate/actions/workflows/ci.yml)
[![Release](https://github.com/hozi8-web3/tollgate/actions/workflows/release.yml/badge.svg)](https://github.com/hozi8-web3/tollgate/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## What It Does

Tollgate is a local reverse proxy. Point your SDK's `base_url` at `localhost:4000`, and it:

1. **Forwards** every request to the real LLM API (Anthropic, OpenAI, Groq)
2. **Captures** the response with exact token counts from the provider
3. **Calculates** costs using per-model pricing rates
4. **Logs** everything to a local SQLite database
5. **Serves** a beautiful real-time dashboard at `localhost:4001`

Zero latency overhead. Zero code changes beyond one URL swap.

## Quick Start

```bash
# Build from source
cargo build --release

# Start proxy + dashboard
./target/release/lct start

# Point your SDK at the proxy
export OPENAI_BASE_URL="http://localhost:4000/openai"
export ANTHROPIC_BASE_URL="http://localhost:4000/anthropic"
```

## SDK Integration (One Line)

```python
# Python + OpenAI
client = OpenAI(base_url="http://localhost:4000/openai")

# Python + Anthropic
client = anthropic.Anthropic(base_url="http://localhost:4000/anthropic")
```

```typescript
// TypeScript + OpenAI
const client = new OpenAI({ baseURL: "http://localhost:4000/openai" });
```

```bash
# Or just set the env var — zero code changes
export OPENAI_BASE_URL="http://localhost:4000/openai"
```

## CLI Commands

```bash
lct start                    # Start proxy + dashboard
lct start --no-dashboard     # Proxy only
lct stats                    # Print 7-day summary
lct stats --days 30 --json   # Machine-readable output
lct export --format csv      # Export logs to CSV
lct export --format json     # Export logs to JSON
lct pricing show             # Show current model pricing
lct reset                    # Wipe the database
```

## Features

- 📡 **Transparent Proxy** — Works with any HTTP-based LLM SDK
- 💰 **Accurate Cost Tracking** — Uses real token counts from API responses
- 📊 **Real-time Dashboard** — Dark-mode UI with charts and insights
- 🔄 **Smart Routing** — Rule-based model substitution for cost savings
- ⚠️ **Anomaly Detection** — Flags requests that cost 3x the rolling average
- 📦 **Cache Tracking** — Monitors prompt caching hit rates and savings
- 🔌 **Multi-Provider** — Anthropic, OpenAI, Groq out of the box
- 📤 **Export** — CSV and JSON export for all logged data

## Supported Providers & Models

| Provider | Models | Input $/1M | Output $/1M |
|----------|--------|-----------|------------|
| Anthropic | Claude Opus 4.6 | $15.00 | $75.00 |
| Anthropic | Claude Sonnet 4.6 | $3.00 | $15.00 |
| Anthropic | Claude Haiku 4.5 | $0.80 | $4.00 |
| OpenAI | GPT-4o | $2.50 | $10.00 |
| OpenAI | GPT-4o-mini | $0.15 | $0.60 |
| Groq | Llama 3.3 70B | $0.59 | $0.79 |
| Groq | Llama 3.1 8B | $0.05 | $0.08 |

## Configuration

```bash
# Copy example config
cp config.example.toml ~/.lct/config.toml
```

Key settings:
- **Proxy port** (default: 4000)
- **Dashboard port** (default: 4001)
- **Cost optimization** — auto-downgrade simple tasks to cheaper models
- **Max cost per request** — block expensive requests
- **Routing rules** — keyword-based model substitution
- **Anomaly alerts** — flag unusual spending patterns

## Architecture

```
Your App → localhost:4000 (Proxy) → LLM API
                ↓
            SQLite DB ← localhost:4001 (Dashboard)
```

Built with Rust, axum, reqwest, rusqlite. Single static binary, no runtime dependencies.

## License

MIT
