use serde::Serialize;

use crate::db::read::{AnomalyStats, CacheStats, ModelBreakdown, PeriodStats, TaskBreakdown};

/// Insights output — generated from aggregated stats.
#[derive(Debug, Serialize, Clone)]
pub struct InsightsOutput {
    pub summary: String,
    pub trend: String,
    pub trend_pct: f64,
    pub top_insight: String,
    pub recommendations: Vec<Recommendation>,
    pub anomaly_note: Option<String>,
    pub cache_note: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Recommendation {
    pub title: String,
    pub detail: String,
    pub saving_usd: Option<f64>,
}

/// Generate insights from aggregated database stats.
/// This is a rule-based insights engine (no LLM call needed).
pub fn generate_insights(
    stats: &PeriodStats,
    models: &[ModelBreakdown],
    tasks: &[TaskBreakdown],
    cache: &CacheStats,
    anomalies: &AnomalyStats,
    period_days: i64,
) -> InsightsOutput {
    let mut recommendations = Vec::new();

    // Trend calculation
    let (trend, trend_pct) = if stats.prev_period_spend_usd > 0.0 {
        let pct =
            ((stats.spend_usd - stats.prev_period_spend_usd) / stats.prev_period_spend_usd) * 100.0;
        let t = if pct > 5.0 {
            "up"
        } else if pct < -5.0 {
            "down"
        } else {
            "stable"
        };
        (t.to_string(), pct)
    } else {
        ("stable".to_string(), 0.0)
    };

    // Summary
    let summary = format!(
        "You spent ${:.2} across {} requests in the last {} days. Spending is {} ({:+.1}%).",
        stats.spend_usd, stats.requests, period_days, trend, trend_pct
    );

    // --- Recommendation triggers ---

    // 1. Expensive models used for cheap tasks
    let cheap_task_types = ["classification", "translation"];
    let expensive_models = ["claude-opus-4-6", "claude-sonnet-4-6", "gpt-4o"];
    for task in tasks {
        if cheap_task_types.contains(&task.task_type.as_str()) && task.spend_usd > 0.10 {
            for model in models {
                if expensive_models.contains(&model.model.as_str()) {
                    let est_savings = task.spend_usd * 0.7; // ~70% savings on cheaper model
                    recommendations.push(Recommendation {
                        title: format!("Downgrade {} tasks", task.task_type),
                        detail: format!(
                            "Use Haiku or GPT-4o-mini for {} instead of {}. Est. saving: ${:.2}/period.",
                            task.task_type, model.model, est_savings
                        ),
                        saving_usd: Some(est_savings),
                    });
                    break;
                }
            }
        }
    }

    // 2. Low cache hit rate
    if cache.cache_hit_rate_pct < 20.0 && cache.total_requests > 30 {
        recommendations.push(Recommendation {
            title: "Enable prompt caching".to_string(),
            detail: format!(
                "Cache hit rate is only {:.0}%. Enable prompt caching to reduce input costs.",
                cache.cache_hit_rate_pct
            ),
            saving_usd: None,
        });
    }

    // 3. Single model dominance
    if let Some(top_model) = models.first() {
        if stats.spend_usd > 0.0 {
            let share = top_model.spend_usd / stats.spend_usd * 100.0;
            if share > 90.0 {
                recommendations.push(Recommendation {
                    title: "Add a fallback model".to_string(),
                    detail: format!(
                        "{} accounts for {:.0}% of spend. Add a fallback for resilience.",
                        top_model.model, share
                    ),
                    saving_usd: None,
                });
            }
        }
    }

    // 4. High latency
    for model in models {
        if model.avg_latency_ms > 5000.0 {
            recommendations.push(Recommendation {
                title: format!("{} latency concern", model.model),
                detail: format!(
                    "Avg latency {:.0}ms may hurt UX. Consider a faster model.",
                    model.avg_latency_ms
                ),
                saving_usd: None,
            });
        }
    }

    // 5. Expensive single requests
    if anomalies.highest_single_request_usd > 0.50 {
        recommendations.push(Recommendation {
            title: "Expensive request detected".to_string(),
            detail: format!(
                "Highest single request cost ${:.2}. Consider setting max_cost_per_request.",
                anomalies.highest_single_request_usd
            ),
            saving_usd: None,
        });
    }

    // Top insight
    let top_insight = if let Some(first_rec) = recommendations.first() {
        first_rec.detail.clone()
    } else if stats.requests > 0 {
        format!(
            "Avg cost per request: ${:.4}",
            stats.spend_usd / stats.requests as f64
        )
    } else {
        "No requests tracked yet.".to_string()
    };

    // Anomaly note
    let anomaly_note = if anomalies.anomalies_count > 0 {
        Some(format!(
            "{} anomalous requests detected this period.",
            anomalies.anomalies_count
        ))
    } else {
        None
    };

    // Cache note
    let cache_note = if cache.total_requests > 0 {
        Some(format!(
            "Cache hit rate: {:.1}%. Est. cache savings: ${:.4}.",
            cache.cache_hit_rate_pct, cache.estimated_cache_savings_usd
        ))
    } else {
        None
    };

    InsightsOutput {
        summary,
        trend,
        trend_pct,
        top_insight,
        recommendations,
        anomaly_note,
        cache_note,
    }
}
