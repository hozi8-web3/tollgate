// ── Tollgate Dashboard — Frontend Logic ──

const API_BASE = '';
let currentPeriod = 7;
let refreshInterval = null;
let countdown = 30;

// ── Init ──
document.addEventListener('DOMContentLoaded', () => {
    setupPeriodSelector();
    fetchAll();
    startAutoRefresh();
});

function setupPeriodSelector() {
    document.querySelectorAll('.period-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelector('.period-btn.active')?.classList.remove('active');
            btn.classList.add('active');
            currentPeriod = parseInt(btn.dataset.days);
            fetchAll();
        });
    });
}

function startAutoRefresh() {
    countdown = 30;
    if (refreshInterval) clearInterval(refreshInterval);
    refreshInterval = setInterval(() => {
        countdown--;
        document.getElementById('refresh-timer').textContent = `Refreshing in ${countdown}s`;
        if (countdown <= 0) {
            fetchAll();
            countdown = 30;
        }
    }, 1000);
}

// ── Data Fetching ──
async function fetchAll() {
    countdown = 30;
    await Promise.all([
        fetchStats(),
        fetchDaily(),
        fetchModels(),
        fetchRequests(),
        fetchInsights(),
    ]);
}

async function fetchJSON(url) {
    try {
        const resp = await fetch(`${API_BASE}${url}`);
        return await resp.json();
    } catch (e) {
        console.error(`Fetch error: ${url}`, e);
        return null;
    }
}

// ── Stats ──
async function fetchStats() {
    const data = await fetchJSON(`/api/stats?days=${currentPeriod}`);
    if (!data || data.error) return;

    document.getElementById('stat-spend').textContent = formatUSD(data.spend_usd);
    document.getElementById('stat-requests').textContent = formatNumber(data.requests);
    document.getElementById('stat-tokens').textContent = formatNumber(data.input_tokens + data.output_tokens);
    document.getElementById('stat-token-split').textContent = `In: ${formatNumber(data.input_tokens)} | Out: ${formatNumber(data.output_tokens)}`;
    document.getElementById('stat-avg-cost').textContent = `Avg: ${formatUSD(data.avg_cost_per_request)}/req`;

    const trendEl = document.getElementById('stat-trend');
    if (data.prev_period_spend_usd > 0) {
        const pct = ((data.spend_usd - data.prev_period_spend_usd) / data.prev_period_spend_usd * 100);
        const dir = pct > 5 ? 'up' : pct < -5 ? 'down' : 'stable';
        const arrow = dir === 'up' ? '↑' : dir === 'down' ? '↓' : '→';
        trendEl.className = `stat-trend ${dir}`;
        trendEl.textContent = `${arrow} ${Math.abs(pct).toFixed(1)}% vs prev period`;
    } else {
        trendEl.className = 'stat-trend stable';
        trendEl.textContent = 'No prior data';
    }
}

// ── Daily Spend Chart ──
async function fetchDaily() {
    const data = await fetchJSON(`/api/daily?days=${currentPeriod}`);
    if (!data || !data.daily) return;
    renderSpendChart(data.daily);
}

function renderSpendChart(daily) {
    const canvas = document.getElementById('spend-chart');
    const ctx = canvas.getContext('2d');
    const rect = canvas.parentElement.getBoundingClientRect();
    canvas.width = rect.width * 2;
    canvas.height = rect.height * 2;
    ctx.scale(2, 2);

    const w = rect.width;
    const h = rect.height;
    const pad = { top: 20, right: 20, bottom: 40, left: 60 };
    const plotW = w - pad.left - pad.right;
    const plotH = h - pad.top - pad.bottom;

    ctx.clearRect(0, 0, w, h);

    if (daily.length === 0) {
        ctx.fillStyle = '#55556a';
        ctx.font = '14px Inter';
        ctx.textAlign = 'center';
        ctx.fillText('No data yet', w / 2, h / 2);
        return;
    }

    const maxSpend = Math.max(...daily.map(d => d.spend_usd), 0.01);
    const xStep = plotW / Math.max(daily.length - 1, 1);

    // Grid lines
    ctx.strokeStyle = 'rgba(255,255,255,0.04)';
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
        const y = pad.top + (plotH / 4) * i;
        ctx.beginPath();
        ctx.moveTo(pad.left, y);
        ctx.lineTo(pad.left + plotW, y);
        ctx.stroke();

        ctx.fillStyle = '#55556a';
        ctx.font = '10px Inter';
        ctx.textAlign = 'right';
        const val = maxSpend * (1 - i / 4);
        ctx.fillText('$' + val.toFixed(2), pad.left - 8, y + 4);
    }

    // Area gradient
    const gradient = ctx.createLinearGradient(0, pad.top, 0, pad.top + plotH);
    gradient.addColorStop(0, 'rgba(99, 102, 241, 0.25)');
    gradient.addColorStop(1, 'rgba(99, 102, 241, 0.0)');

    ctx.beginPath();
    ctx.moveTo(pad.left, pad.top + plotH);
    daily.forEach((d, i) => {
        const x = pad.left + i * xStep;
        const y = pad.top + plotH - (d.spend_usd / maxSpend) * plotH;
        ctx.lineTo(x, y);
    });
    ctx.lineTo(pad.left + (daily.length - 1) * xStep, pad.top + plotH);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();

    // Line
    ctx.beginPath();
    daily.forEach((d, i) => {
        const x = pad.left + i * xStep;
        const y = pad.top + plotH - (d.spend_usd / maxSpend) * plotH;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    });
    ctx.strokeStyle = '#818cf8';
    ctx.lineWidth = 2.5;
    ctx.lineJoin = 'round';
    ctx.stroke();

    // Dots
    daily.forEach((d, i) => {
        const x = pad.left + i * xStep;
        const y = pad.top + plotH - (d.spend_usd / maxSpend) * plotH;
        ctx.beginPath();
        ctx.arc(x, y, 3.5, 0, Math.PI * 2);
        ctx.fillStyle = '#818cf8';
        ctx.fill();
        ctx.strokeStyle = '#0a0a0f';
        ctx.lineWidth = 2;
        ctx.stroke();
    });

    // X-axis labels
    ctx.fillStyle = '#55556a';
    ctx.font = '10px Inter';
    ctx.textAlign = 'center';
    const labelStep = Math.max(1, Math.floor(daily.length / 7));
    daily.forEach((d, i) => {
        if (i % labelStep === 0 || i === daily.length - 1) {
            const x = pad.left + i * xStep;
            const label = d.date.slice(5); // MM-DD
            ctx.fillText(label, x, pad.top + plotH + 20);
        }
    });
}

// ── Models ──
async function fetchModels() {
    const data = await fetchJSON(`/api/models?days=${currentPeriod}`);
    if (!data || !data.models) return;

    const container = document.getElementById('model-list');
    if (data.models.length === 0) {
        container.innerHTML = '<div class="empty-state">No data yet</div>';
        return;
    }

    const maxSpend = Math.max(...data.models.map(m => m.spend_usd), 0.01);
    container.innerHTML = data.models.map(m => `
        <div class="model-item">
            <div class="model-info">
                <span class="model-name">${escapeHtml(m.model)}</span>
                <span class="model-provider">${escapeHtml(m.provider)}</span>
            </div>
            <div class="model-stats">
                <div class="model-cost">${formatUSD(m.spend_usd)}</div>
                <div class="model-reqs">${m.requests} reqs · ${Math.round(m.avg_latency_ms)}ms</div>
            </div>
        </div>
        <div class="model-bar"><div class="model-bar-fill" style="width:${(m.spend_usd/maxSpend*100).toFixed(1)}%"></div></div>
    `).join('');
}

// ── Request Log ──
async function fetchRequests() {
    const data = await fetchJSON('/api/requests?limit=50&offset=0');
    if (!data || !data.requests) return;

    document.getElementById('log-count').textContent = data.requests.length;

    const tbody = document.getElementById('log-body');
    if (data.requests.length === 0) {
        tbody.innerHTML = '<tr><td colspan="8" class="empty-state">No requests tracked yet. Point your SDK at localhost:4000.</td></tr>';
        return;
    }

    tbody.innerHTML = data.requests.map(r => {
        const time = new Date(r.timestamp).toLocaleTimeString();
        const tokens = `${formatNumber(r.input_tokens)}→${formatNumber(r.output_tokens)}`;
        const latency = `${r.latency_ms}ms`;
        let status = '';
        if (r.anomaly) status += '<span class="anomaly-badge">⚠ Anomaly</span> ';
        if (r.was_substituted) status += '<span class="sub-badge">↔ Subst</span>';
        if (!status) status = '<span style="color: var(--accent-emerald)">✓</span>';

        return `<tr>
            <td>${time}</td>
            <td><span class="provider-badge provider-${escapeHtml(r.provider)}">${escapeHtml(r.provider)}</span></td>
            <td>${escapeHtml(r.model)}</td>
            <td>${escapeHtml(r.task_type || 'other')}</td>
            <td>${tokens}</td>
            <td class="cost-cell">${formatUSD(r.total_cost_usd)}</td>
            <td>${latency}</td>
            <td>${status}</td>
        </tr>`;
    }).join('');
}

// ── Insights ──
async function fetchInsights() {
    const data = await fetchJSON(`/api/insights?days=${currentPeriod}`);
    if (!data || data.error) return;

    document.getElementById('stat-top-insight').textContent = data.top_insight || 'No insights yet';

    const container = document.getElementById('insights-content');
    let html = '';

    if (data.summary) {
        html += `<div class="insight-summary">${escapeHtml(data.summary)}</div>`;
    }

    if (data.recommendations && data.recommendations.length > 0) {
        html += '<div class="recommendations-grid">';
        data.recommendations.forEach(r => {
            html += `<div class="rec-card">
                <div class="rec-title">${escapeHtml(r.title)}</div>
                <div class="rec-detail">${escapeHtml(r.detail)}</div>
                ${r.saving_usd ? `<div class="rec-saving">Est. saving: ${formatUSD(r.saving_usd)}</div>` : ''}
            </div>`;
        });
        html += '</div>';
    }

    if (data.anomaly_note) {
        html += `<div class="insight-note">⚠ ${escapeHtml(data.anomaly_note)}</div>`;
    }
    if (data.cache_note) {
        html += `<div class="insight-note">📦 ${escapeHtml(data.cache_note)}</div>`;
    }

    if (!html) html = '<div class="empty-state">No insights available yet. Start making API calls!</div>';
    container.innerHTML = html;
}

// ── Utilities ──
function formatUSD(val) {
    if (val === undefined || val === null) return '$0.00';
    if (val < 0.01 && val > 0) return '$' + val.toFixed(4);
    return '$' + val.toFixed(2);
}

function formatNumber(n) {
    if (n === undefined || n === null) return '0';
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
    return n.toLocaleString();
}

function escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

// Handle window resize for chart
window.addEventListener('resize', () => {
    fetchDaily();
});
