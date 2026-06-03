// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Admin UI  (served as inline HTML string)
// ─────────────────────────────────────────────────────────────────────────────
export const ADMIN_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<title>PrxyClaude · Admin</title>
<link rel="preconnect" href="https://fonts.googleapis.com"/>
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin/>
<link href="https://fonts.googleapis.com/css2?family=Syne+Mono&family=DM+Mono:ital,wght@0,300;0,400;0,500;1,400&family=Syne:wght@700;800&display=swap" rel="stylesheet"/>
<style>
:root{
  --bg:#070709;--surface:#0e0f13;--surface2:#161820;--border:#252630;
  --text:#e8e9f0;--dim:#6b6d82;--accent:#7cffa4;--accent2:#ff7c7c;
  --amber:#ffb347;--blue:#7cb8ff;--purple:#c87cff;
  --font-mono:"DM Mono",monospace;--font-head:"Syne",sans-serif;
  --radius:6px;--transition:180ms ease;
}
*{box-sizing:border-box;margin:0;padding:0}
html{font-size:14px}
body{background:var(--bg);color:var(--text);font-family:var(--font-mono);min-height:100vh;overflow-x:hidden}

/* ── GRID NOISE OVERLAY ── */
body::before{
  content:"";position:fixed;inset:0;
  background-image:url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.04'/%3E%3C/svg%3E");
  pointer-events:none;z-index:0;opacity:.4;
}

/* ── HEADER ── */
header{
  position:sticky;top:0;z-index:100;
  background:rgba(7,7,9,.92);backdrop-filter:blur(12px);
  border-bottom:1px solid var(--border);
  padding:0 28px;height:52px;
  display:flex;align-items:center;justify-content:space-between;
}
.logo{font-family:var(--font-head);font-size:1.1rem;font-weight:800;letter-spacing:-.5px;
  display:flex;align-items:center;gap:8px}
.logo-dot{width:8px;height:8px;border-radius:50%;background:var(--accent);
  animation:pulse 2s ease-in-out infinite}
@keyframes pulse{0%,100%{box-shadow:0 0 0 0 rgba(124,255,164,.6)}50%{box-shadow:0 0 0 6px rgba(124,255,164,0)}}
.header-right{display:flex;align-items:center;gap:16px}
.uptime{color:var(--dim);font-size:.8rem}
.refresh-btn{
  background:transparent;border:1px solid var(--border);color:var(--dim);
  padding:5px 12px;border-radius:var(--radius);cursor:pointer;font:inherit;font-size:.8rem;
  transition:var(--transition);
}
.refresh-btn:hover{border-color:var(--accent);color:var(--accent)}

/* ── LAYOUT ── */
main{max-width:1400px;margin:0 auto;padding:28px;position:relative;z-index:1}

/* ── STAT CARDS ── */
.stat-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:12px;margin-bottom:28px}
.stat-card{
  background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);
  padding:16px 18px;
  transition:border-color var(--transition);
}
.stat-card:hover{border-color:var(--accent)}
.stat-label{font-size:.72rem;color:var(--dim);text-transform:uppercase;letter-spacing:.08em;margin-bottom:6px}
.stat-value{font-family:var(--font-head);font-size:1.7rem;font-weight:800;color:var(--text);line-height:1}
.stat-sub{font-size:.75rem;color:var(--dim);margin-top:4px}

/* ── SECTION ── */
.section{margin-bottom:28px}
.section-header{
  display:flex;align-items:center;justify-content:space-between;
  margin-bottom:12px;padding-bottom:8px;border-bottom:1px solid var(--border);
}
.section-title{font-family:var(--font-head);font-size:.85rem;font-weight:800;
  text-transform:uppercase;letter-spacing:.12em;color:var(--dim)}

/* ── PROVIDER TABLE ── */
.provider-table{width:100%;border-collapse:collapse}
.provider-table th{
  font-size:.7rem;text-transform:uppercase;letter-spacing:.1em;
  color:var(--dim);text-align:left;padding:8px 12px;
  border-bottom:1px solid var(--border);
}
.provider-table td{padding:10px 12px;border-bottom:1px solid rgba(37,38,48,.6)}
.provider-table tr:hover td{background:rgba(255,255,255,.015)}

/* ── BADGES ── */
.badge{
  display:inline-flex;align-items:center;gap:4px;
  padding:2px 8px;border-radius:2px;font-size:.7rem;
  font-weight:500;letter-spacing:.04em;
}
.badge-green{background:rgba(124,255,164,.12);color:var(--accent);border:1px solid rgba(124,255,164,.25)}
.badge-red  {background:rgba(255,124,124,.12);color:var(--accent2);border:1px solid rgba(255,124,124,.25)}
.badge-amber{background:rgba(255,179,71,.12) ;color:var(--amber) ;border:1px solid rgba(255,179,71,.25)}
.badge-blue {background:rgba(124,184,255,.12);color:var(--blue)  ;border:1px solid rgba(124,184,255,.25)}
.badge-dim  {background:rgba(107,109,130,.1) ;color:var(--dim)   ;border:1px solid rgba(107,109,130,.2)}
.dot{width:6px;height:6px;border-radius:50%;background:currentColor}

/* ── PROGRESS BAR ── */
.bar-track{height:4px;background:var(--border);border-radius:2px;overflow:hidden}
.bar-fill{height:100%;border-radius:2px;transition:width .4s ease;background:var(--accent)}
.bar-fill.warn{background:var(--amber)}
.bar-fill.crit{background:var(--accent2)}

/* ── ACTION BTN ── */
.action{
  background:transparent;border:1px solid var(--border);color:var(--dim);
  padding:3px 10px;border-radius:var(--radius);cursor:pointer;font:inherit;font-size:.72rem;
  transition:var(--transition);white-space:nowrap;
}
.action:hover{border-color:var(--accent);color:var(--accent)}
.action.danger:hover{border-color:var(--accent2);color:var(--accent2)}
.action-group{display:flex;gap:6px}

/* ── CIRCUIT RING ── */
.circuit-ring{
  width:38px;height:38px;border-radius:50%;
  display:flex;align-items:center;justify-content:center;
  font-size:.62rem;font-weight:500;text-transform:uppercase;letter-spacing:.04em;
  border:2px solid;
}
.ring-closed  {border-color:var(--accent);color:var(--accent)}
.ring-open    {border-color:var(--accent2);color:var(--accent2)}
.ring-half-open{border-color:var(--amber);color:var(--amber)}

/* ── METRICS ROW ── */
.metrics-row{display:flex;gap:8px;font-size:.75rem;color:var(--dim)}
.metric-pill{display:flex;gap:4px;align-items:center}
.metric-pill span:last-child{color:var(--text)}

/* ── TOAST ── */
#toast{
  position:fixed;bottom:24px;right:24px;z-index:999;
  background:var(--surface2);border:1px solid var(--accent);color:var(--accent);
  padding:10px 18px;border-radius:var(--radius);font-size:.8rem;
  transform:translateY(20px);opacity:0;
  transition:all .25s ease;pointer-events:none;
}
#toast.show{transform:translateY(0);opacity:1}

/* ── TWO-COL ── */
.two-col{display:grid;grid-template-columns:1fr 1fr;gap:20px}
@media(max-width:900px){.two-col{grid-template-columns:1fr}}

/* ── LOG BOX ── */
.log-box{
  background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);
  padding:12px;max-height:220px;overflow-y:auto;font-size:.75rem;
  line-height:1.6;color:var(--dim);
}
.log-line.info {color:var(--blue)}
.log-line.warn {color:var(--amber)}
.log-line.error{color:var(--accent2)}
.log-line.ok   {color:var(--accent)}

/* ── QUEUE VISUAL ── */
.queue-bar{
  height:28px;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius);overflow:hidden;display:flex;align-items:center;
  padding:0 10px;gap:6px;
}
.queue-fill{
  position:absolute;left:0;top:0;height:100%;
  background:linear-gradient(90deg,rgba(124,255,164,.15),rgba(124,184,255,.15));
  border-radius:var(--radius);transition:width .4s ease;
}
.queue-bar{position:relative}

/* ── KEY SLOTS ── */
.key-slots{display:flex;flex-wrap:wrap;gap:6px;margin-top:4px}
.key-slot{
  width:28px;height:28px;border-radius:4px;border:1px solid;
  display:flex;align-items:center;justify-content:center;font-size:.7rem;
  transition:var(--transition);
}
.key-slot.active{border-color:var(--accent);color:var(--accent);background:rgba(124,255,164,.06)}
.key-slot.banned{border-color:var(--accent2);color:var(--accent2);background:rgba(255,124,124,.06)}
.key-slot.idle  {border-color:var(--border);color:var(--dim)}
</style>
</head>
<body>

<header>
  <div class="logo">
    <div class="logo-dot"></div>
    PrxyClaude
    <span style="color:var(--dim);font-weight:400;font-size:.8rem;margin-left:4px">/ admin</span>
  </div>
  <div class="header-right">
    <span class="uptime" id="uptimeEl">uptime: —</span>
    <button class="refresh-btn" onclick="refresh()">⟳ Refresh</button>
  </div>
</header>

<main>

  <!-- STAT CARDS -->
  <div class="stat-grid" id="statGrid">
    <div class="stat-card">
      <div class="stat-label">Total Requests</div>
      <div class="stat-value" id="statTotal">—</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Cache Hits</div>
      <div class="stat-value" id="statCache">—</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Queue Depth</div>
      <div class="stat-value" id="statQueue">—</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Active Providers</div>
      <div class="stat-value" id="statProviders">—</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Tokens In</div>
      <div class="stat-value" id="statTokensIn">—</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Tokens Out</div>
      <div class="stat-value" id="statTokensOut">—</div>
    </div>
  </div>

  <!-- PROVIDERS -->
  <div class="section">
    <div class="section-header">
      <span class="section-title">Providers</span>
    </div>
    <table class="provider-table" id="providerTable">
      <thead>
        <tr>
          <th>Circuit</th>
          <th>Provider</th>
          <th>Status</th>
          <th>Keys</th>
          <th>Requests</th>
          <th>Success Rate</th>
          <th>Avg Latency</th>
          <th>Tokens</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody id="providerTbody">
        <tr><td colspan="9" style="color:var(--dim);padding:20px;text-align:center">Loading…</td></tr>
      </tbody>
    </table>
  </div>

  <!-- QUEUE + CACHE -->
  <div class="two-col">
    <div class="section">
      <div class="section-header">
        <span class="section-title">Request Queue</span>
      </div>
      <div style="background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:16px">
        <div class="queue-bar" id="queueBar">
          <div class="queue-fill" id="queueFill" style="width:0%"></div>
          <span style="position:relative;font-size:.75rem;color:var(--text)" id="queueLabel">0 / 0</span>
        </div>
        <div class="metrics-row" style="margin-top:12px" id="queueMeta">
          <div class="metric-pill">Active: <span>—</span></div>
          <div class="metric-pill">Max Concurrent: <span>—</span></div>
        </div>
      </div>
    </div>

    <div class="section">
      <div class="section-header">
        <span class="section-title">Response Cache</span>
        <button class="action danger" onclick="clearCache()">Clear</button>
      </div>
      <div style="background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:16px">
        <div id="cacheInfo">
          <div class="bar-track" style="margin-bottom:10px">
            <div class="bar-fill" id="cacheFill" style="width:0%"></div>
          </div>
          <div class="metrics-row" id="cacheMeta">
            <div class="metric-pill">Entries: <span id="cacheSize">—</span></div>
            <div class="metric-pill">Max: <span id="cacheMax">—</span></div>
            <div class="metric-pill">TTL: <span id="cacheTtl">—</span></div>
            <div class="metric-pill">Hits: <span id="cacheHits">—</span></div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- CIRCUIT BREAKERS -->
  <div class="section">
    <div class="section-header">
      <span class="section-title">Circuit Breakers</span>
    </div>
    <div id="circuitGrid" style="display:flex;flex-wrap:wrap;gap:10px">
      <span style="color:var(--dim);font-size:.8rem">Loading…</span>
    </div>
  </div>

</main>

<div id="toast">Action completed</div>

<script>
const BASE = window.location.origin;
let lastData = null;

async function api(path, opts = {}) {
  const token = localStorage.getItem('adminToken') || '';
  const res = await fetch(BASE + '/admin/api' + path, {
    ...opts,
    headers: { 'Content-Type': 'application/json', 'x-admin-token': token, ...opts.headers },
    body: opts.body ? JSON.stringify(opts.body) : undefined,
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

function toast(msg, isError = false) {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.style.borderColor = isError ? 'var(--accent2)' : 'var(--accent)';
  el.style.color = isError ? 'var(--accent2)' : 'var(--accent)';
  el.classList.add('show');
  setTimeout(() => el.classList.remove('show'), 2500);
}

function fmt(n) {
  if (n === undefined || n === null) return '—';
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
  return String(n);
}

function renderProviders(data) {
  const tbody = document.getElementById('providerTbody');
  const providers = data.providers || [];
  const metrics = data.metrics?.providers || {};
  const circuits = data.circuits || [];
  const keyStats = data.keys || {};

  if (!providers.length) {
    tbody.innerHTML = '<tr><td colspan="9" style="color:var(--dim);padding:20px;text-align:center">No providers configured. Set env vars and restart.</td></tr>';
    return;
  }

  tbody.innerHTML = providers.map(p => {
    const m = metrics[p.id] || {};
    const c = circuits.find(cb => cb.providerId === p.id) || { state: 'closed', failures: 0 };
    const keys = keyStats[p.id] || [];
    const totalReq = m.requests || 0;
    const succ = m.successes || 0;
    const rate = totalReq > 0 ? Math.round((succ / totalReq) * 100) : 100;
    const rateColor = rate >= 90 ? 'var(--accent)' : rate >= 70 ? 'var(--amber)' : 'var(--accent2)';
    const latency = m.avgLatencyMs ? Math.round(m.avgLatencyMs) + 'ms' : '—';

    const circClass = c.state === 'closed' ? 'ring-closed' : c.state === 'open' ? 'ring-open' : 'ring-half-open';
    const circLabel = c.state === 'closed' ? '✓' : c.state === 'open' ? '✗' : '~';

    const enabledBadge = p.enabled
      ? '<span class="badge badge-green"><span class="dot"></span>enabled</span>'
      : '<span class="badge badge-dim"><span class="dot"></span>disabled</span>';

    const tokensTotal = fmt((m.totalTokensIn || 0) + (m.totalTokensOut || 0));

    const keySlots = keys.length
      ? keys.map((k, i) => {
          const banned = k.bannedUntil && k.bannedUntil > Date.now();
          const cls = banned ? 'banned' : k.usageCount > 0 ? 'active' : 'idle';
          return \`<div class="key-slot \${cls}" title="Key #\${i+1}: \${banned ? 'banned' : k.usageCount + ' uses'}">\${i+1}</div>\`;
        }).join('')
      : '<span style="color:var(--dim);font-size:.7rem">no keys</span>';

    return \`<tr>
      <td><div class="circuit-ring \${circClass}">\${circLabel}</div></td>
      <td>
        <div style="font-weight:500">\${p.label}</div>
        <div style="font-size:.7rem;color:var(--dim)">\${p.type}</div>
      </td>
      <td>\${enabledBadge}</td>
      <td><div class="key-slots">\${keySlots}</div></td>
      <td>
        <div>\${fmt(totalReq)}</div>
        <div style="font-size:.7rem;color:var(--dim)">\${succ} ok · \${m.failures || 0} err</div>
      </td>
      <td>
        <div style="color:\${rateColor}">\${totalReq > 0 ? rate + '%' : '—'}</div>
        <div class="bar-track" style="width:80px;margin-top:4px">
          <div class="bar-fill \${rate < 90 ? (rate < 70 ? 'crit' : 'warn') : ''}" style="width:\${rate}%"></div>
        </div>
      </td>
      <td style="color:\${parseInt(latency) > 3000 ? 'var(--amber)' : 'inherit'}">\${latency}</td>
      <td style="font-size:.75rem;color:var(--dim)">\${tokensTotal}</td>
      <td>
        <div class="action-group">
          <button class="action" onclick="resetCircuit('\${p.id}')">Reset CB</button>
          <button class="action \${p.enabled ? 'danger' : ''}" 
            onclick="\${p.enabled ? 'disableProvider' : 'enableProvider'}('\${p.id}')">
            \${p.enabled ? 'Disable' : 'Enable'}
          </button>
        </div>
      </td>
    </tr>\`;
  }).join('');
}

function renderCircuits(data) {
  const circuits = data.circuits || [];
  const grid = document.getElementById('circuitGrid');
  if (!circuits.length) { grid.innerHTML = '<span style="color:var(--dim);font-size:.8rem">No circuits registered yet.</span>'; return; }
  grid.innerHTML = circuits.map(c => {
    const cls = c.state === 'closed' ? 'badge-green' : c.state === 'open' ? 'badge-red' : 'badge-amber';
    const retryIn = c.retryAfter ? Math.max(0, Math.round((c.retryAfter - Date.now()) / 1000)) : null;
    return \`<div style="background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:12px 16px;min-width:160px">
      <div style="font-size:.75rem;color:var(--dim);margin-bottom:6px">\${c.providerId}</div>
      <div class="badge \${cls}" style="margin-bottom:8px"><span class="dot"></span>\${c.state}</div>
      <div style="font-size:.7rem;color:var(--dim)">
        failures: <span style="color:var(--text)">\${c.failures}</span> /
        total: <span style="color:var(--text)">\${c.totalFailures}</span>
        \${retryIn !== null ? '<br>retry in: <span style="color:var(--amber)">' + retryIn + 's</span>' : ''}
      </div>
    </div>\`;
  }).join('');
}

function updateStats(data) {
  const m = data.metrics || {};
  const q = data.queue || {};
  document.getElementById('statTotal').textContent = fmt(m.totalRequests);
  document.getElementById('statCache').textContent = fmt(m.cachedRequests);
  document.getElementById('statQueue').textContent = fmt(q.depth);
  
  const activeProviders = (data.providers || []).filter(p => p.enabled).length;
  document.getElementById('statProviders').textContent = activeProviders;

  let tokIn = 0, tokOut = 0;
  Object.values(m.providers || {}).forEach(p => {
    tokIn += p.totalTokensIn || 0;
    tokOut += p.totalTokensOut || 0;
  });
  document.getElementById('statTokensIn').textContent = fmt(tokIn);
  document.getElementById('statTokensOut').textContent = fmt(tokOut);

  // Queue bar
  const qFill = q.maxSize > 0 ? Math.min(100, (q.depth / q.maxSize) * 100) : 0;
  document.getElementById('queueFill').style.width = qFill + '%';
  document.getElementById('queueLabel').textContent = q.depth + ' / ' + q.maxSize;
  const qMeta = document.getElementById('queueMeta');
  qMeta.innerHTML = \`<div class="metric-pill">Active: <span>\${q.active || 0}</span></div>
    <div class="metric-pill">Max Concurrent: <span>\${q.maxConcurrent || 0}</span></div>\`;

  // Cache
  const c = data.cache || {};
  const cFill = c.maxEntries > 0 ? Math.min(100, (c.size / c.maxEntries) * 100) : 0;
  document.getElementById('cacheFill').style.width = cFill + '%';
  document.getElementById('cacheFill').className = 'bar-fill' + (cFill > 80 ? ' warn' : '');
  document.getElementById('cacheSize').textContent = fmt(c.size);
  document.getElementById('cacheMax').textContent = fmt(c.maxEntries);
  document.getElementById('cacheTtl').textContent = c.ttlMs ? Math.round(c.ttlMs / 1000) + 's' : '—';
  
  let totalCacheHits = 0;
  Object.values(m.providers || {}).forEach(p => totalCacheHits += p.cachedHits || 0);
  document.getElementById('cacheHits').textContent = fmt(m.cachedRequests || 0);

  // Uptime
  const ups = data.uptime || 0;
  const h = Math.floor(ups / 3600), mn = Math.floor((ups % 3600) / 60), s = ups % 60;
  document.getElementById('uptimeEl').textContent = \`uptime: \${h}h \${mn}m \${s}s\`;
}

async function refresh() {
  try {
    const data = await api('/status');
    lastData = data;
    updateStats(data);
    renderProviders(data);
    renderCircuits(data);
  } catch(e) {
    toast('Failed to load: ' + e.message, true);
  }
}

async function resetCircuit(id) {
  try {
    await api(\`/provider/\${id}/reset-circuit\`, { method: 'POST' });
    toast(\`Circuit reset: \${id}\`);
    refresh();
  } catch(e) { toast(e.message, true); }
}

async function disableProvider(id) {
  try {
    await api(\`/provider/\${id}/disable\`, { method: 'POST' });
    toast(\`Disabled: \${id}\`);
    refresh();
  } catch(e) { toast(e.message, true); }
}

async function enableProvider(id) {
  try {
    await api(\`/provider/\${id}/enable\`, { method: 'POST' });
    toast(\`Enabled: \${id}\`);
    refresh();
  } catch(e) { toast(e.message, true); }
}

async function clearCache() {
  try {
    await api('/cache/clear', { method: 'POST' });
    toast('Cache cleared');
    refresh();
  } catch(e) { toast(e.message, true); }
}

// Check for token in URL hash on first load
const hashToken = new URLSearchParams(window.location.hash.slice(1)).get('token');
if (hashToken) localStorage.setItem('adminToken', hashToken);

// Auto-refresh every 5 seconds
refresh();
setInterval(refresh, 5000);
</script>
</body>
</html>`;
//# sourceMappingURL=ui.js.map