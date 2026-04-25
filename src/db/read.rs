use anyhow::Result;
use serde::Serialize;
use super::DbPool;
use super::write::RequestRow;

#[derive(Debug, Serialize, Clone)]
pub struct PeriodStats {
    pub spend_usd: f64,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub prev_period_spend_usd: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelBreakdown {
    pub model: String,
    pub provider: String,
    pub requests: i64,
    pub spend_usd: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct TaskBreakdown {
    pub task_type: String,
    pub requests: i64,
    pub spend_usd: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DailySpend {
    pub date: String,
    pub spend_usd: f64,
    pub requests: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct AnomalyStats {
    pub anomalies_count: i64,
    pub highest_single_request_usd: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct CacheStats {
    pub total_requests: i64,
    pub cache_hits: i64,
    pub cache_hit_rate_pct: f64,
    pub estimated_cache_savings_usd: f64,
}

pub fn get_stats(db: &DbPool, period_days: i64) -> Result<PeriodStats> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let offset = format!("-{} days", period_days);
    let mut stmt = conn.prepare(
        "SELECT COALESCE(SUM(total_cost_usd),0), COUNT(*),
                COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0)
         FROM requests WHERE timestamp >= datetime('now', ?1)"
    )?;
    let (spend, reqs, inp, outp) = stmt.query_row([&offset], |r| {
        Ok((r.get::<_,f64>(0)?, r.get::<_,i64>(1)?, r.get::<_,i64>(2)?, r.get::<_,i64>(3)?))
    })?;
    let prev_offset = format!("-{} days", period_days * 2);
    let prev: f64 = conn.query_row(
        "SELECT COALESCE(SUM(total_cost_usd),0) FROM requests
         WHERE timestamp >= datetime('now', ?1) AND timestamp < datetime('now', ?2)",
        [&prev_offset, &offset], |r| r.get(0))?;
    Ok(PeriodStats { spend_usd: spend, requests: reqs, input_tokens: inp, output_tokens: outp, prev_period_spend_usd: prev })
}

pub fn get_model_breakdown(db: &DbPool, period_days: i64) -> Result<Vec<ModelBreakdown>> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let offset = format!("-{} days", period_days);
    let mut stmt = conn.prepare(
        "SELECT model, provider, COUNT(*), COALESCE(SUM(total_cost_usd),0), COALESCE(AVG(latency_ms),0)
         FROM requests WHERE timestamp >= datetime('now', ?1)
         GROUP BY model, provider ORDER BY SUM(total_cost_usd) DESC")?;
    let rows = stmt.query_map([&offset], |r| {
        Ok(ModelBreakdown { model: r.get(0)?, provider: r.get(1)?, requests: r.get(2)?, spend_usd: r.get(3)?, avg_latency_ms: r.get(4)? })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_task_breakdown(db: &DbPool, period_days: i64) -> Result<Vec<TaskBreakdown>> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let offset = format!("-{} days", period_days);
    let mut stmt = conn.prepare(
        "SELECT COALESCE(task_type,'unknown'), COUNT(*), COALESCE(SUM(total_cost_usd),0)
         FROM requests WHERE timestamp >= datetime('now', ?1)
         GROUP BY task_type ORDER BY SUM(total_cost_usd) DESC")?;
    let rows = stmt.query_map([&offset], |r| {
        Ok(TaskBreakdown { task_type: r.get(0)?, requests: r.get(1)?, spend_usd: r.get(2)? })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_daily_spend(db: &DbPool, period_days: i64) -> Result<Vec<DailySpend>> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let offset = format!("-{} days", period_days);
    let mut stmt = conn.prepare(
        "SELECT date(timestamp), COALESCE(SUM(total_cost_usd),0), COUNT(*)
         FROM requests WHERE timestamp >= datetime('now', ?1)
         GROUP BY date(timestamp) ORDER BY date(timestamp) ASC")?;
    let rows = stmt.query_map([&offset], |r| {
        Ok(DailySpend { date: r.get(0)?, spend_usd: r.get(1)?, requests: r.get(2)? })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_requests(db: &DbPool, limit: i64, offset_val: i64) -> Result<Vec<RequestRow>> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id,timestamp,provider,model,original_model,was_substituted,
                input_tokens,output_tokens,cache_read_tokens,cache_write_tokens,
                input_cost_usd,output_cost_usd,cache_read_cost_usd,cache_write_cost_usd,
                total_cost_usd,latency_ms,stop_reason,task_type,tags,anomaly,anomaly_reason
         FROM requests ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2")?;
    let rows = stmt.query_map([limit, offset_val], |r| {
        Ok(RequestRow {
            id: r.get(0)?, timestamp: r.get(1)?, provider: r.get(2)?, model: r.get(3)?,
            original_model: r.get(4)?, was_substituted: r.get::<_,i32>(5)? != 0,
            input_tokens: r.get(6)?, output_tokens: r.get(7)?,
            cache_read_tokens: r.get(8)?, cache_write_tokens: r.get(9)?,
            input_cost_usd: r.get(10)?, output_cost_usd: r.get(11)?,
            cache_read_cost_usd: r.get(12)?, cache_write_cost_usd: r.get(13)?,
            total_cost_usd: r.get(14)?, latency_ms: r.get(15)?,
            stop_reason: r.get(16)?, task_type: r.get(17)?, tags: r.get(18)?,
            anomaly: r.get::<_,i32>(19)? != 0, anomaly_reason: r.get(20)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_rolling_avg_cost(db: &DbPool) -> Result<f64> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let avg: f64 = conn.query_row(
        "SELECT COALESCE(AVG(total_cost_usd),0) FROM requests WHERE timestamp >= datetime('now','-7 days')",
        [], |r| r.get(0))?;
    Ok(avg)
}

pub fn get_anomaly_stats(db: &DbPool, period_days: i64) -> Result<AnomalyStats> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let offset = format!("-{} days", period_days);
    let mut stmt = conn.prepare(
        "SELECT COALESCE(SUM(anomaly),0), COALESCE(MAX(total_cost_usd),0) FROM requests WHERE timestamp >= datetime('now', ?1)")?;
    let (c, h) = stmt.query_row([&offset], |r| Ok((r.get::<_,i64>(0)?, r.get::<_,f64>(1)?)))?;
    Ok(AnomalyStats { anomalies_count: c, highest_single_request_usd: h })
}

pub fn get_cache_stats(db: &DbPool, period_days: i64) -> Result<CacheStats> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let offset = format!("-{} days", period_days);
    let mut stmt = conn.prepare(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN cache_read_tokens>0 THEN 1 ELSE 0 END),0), COALESCE(SUM(cache_read_cost_usd),0)
         FROM requests WHERE timestamp >= datetime('now', ?1)")?;
    let (total, hits, savings) = stmt.query_row([&offset], |r| Ok((r.get::<_,i64>(0)?, r.get::<_,i64>(1)?, r.get::<_,f64>(2)?)))?;
    let rate = if total > 0 { (hits as f64 / total as f64) * 100.0 } else { 0.0 };
    Ok(CacheStats { total_requests: total, cache_hits: hits, cache_hit_rate_pct: rate, estimated_cache_savings_usd: savings })
}

pub fn export_all(db: &DbPool) -> Result<Vec<RequestRow>> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id,timestamp,provider,model,original_model,was_substituted,
                input_tokens,output_tokens,cache_read_tokens,cache_write_tokens,
                input_cost_usd,output_cost_usd,cache_read_cost_usd,cache_write_cost_usd,
                total_cost_usd,latency_ms,stop_reason,task_type,tags,anomaly,anomaly_reason
         FROM requests ORDER BY timestamp ASC")?;
    let rows = stmt.query_map([], |r| {
        Ok(RequestRow {
            id: r.get(0)?, timestamp: r.get(1)?, provider: r.get(2)?, model: r.get(3)?,
            original_model: r.get(4)?, was_substituted: r.get::<_,i32>(5)? != 0,
            input_tokens: r.get(6)?, output_tokens: r.get(7)?,
            cache_read_tokens: r.get(8)?, cache_write_tokens: r.get(9)?,
            input_cost_usd: r.get(10)?, output_cost_usd: r.get(11)?,
            cache_read_cost_usd: r.get(12)?, cache_write_cost_usd: r.get(13)?,
            total_cost_usd: r.get(14)?, latency_ms: r.get(15)?,
            stop_reason: r.get(16)?, task_type: r.get(17)?, tags: r.get(18)?,
            anomaly: r.get::<_,i32>(19)? != 0, anomaly_reason: r.get(20)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn reset_db(db: &DbPool) -> Result<()> {
    let conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock: {}", e))?;
    conn.execute("DELETE FROM requests", [])?;
    tracing::info!("Database reset");
    Ok(())
}
