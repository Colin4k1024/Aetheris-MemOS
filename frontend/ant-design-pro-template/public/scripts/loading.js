/**
 * Static landing page fallback
 * Renders immediately before React loads. If React fails, users still see a functional page.
 * When React mounts successfully, it replaces #root content.
 */
(function () {
  var _root = document.querySelector('#root');
  if (_root && _root.innerHTML === '') {
    _root.innerHTML = '\
<style>\
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}\
:root{--primary:#1677ff;--primary-hover:#4096ff;--bg:#f5f7fa;--card-bg:#fff;--text:#1f2937;--text-secondary:#6b7280;--border:#e5e7eb;--shadow:0 2px 8px rgba(0,0,0,0.06)}\
html,body,#root{height:100%;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"Helvetica Neue",sans-serif;color:var(--text);background:var(--bg);line-height:1.6}\
.sp-header{position:sticky;top:0;z-index:100;display:flex;align-items:center;justify-content:space-between;padding:0 32px;height:56px;background:rgba(255,255,255,0.95);backdrop-filter:blur(8px);border-bottom:1px solid var(--border)}\
.sp-logo{font-size:18px;font-weight:700;color:var(--primary);display:flex;align-items:center;gap:8px}\
.sp-nav{display:flex;align-items:center;gap:20px}\
.sp-nav a{color:var(--text-secondary);text-decoration:none;font-size:14px;transition:color .2s}\
.sp-nav a:hover{color:var(--primary)}\
.sp-btn{display:inline-flex;align-items:center;gap:6px;padding:8px 20px;border-radius:6px;font-size:14px;font-weight:500;text-decoration:none;transition:all .2s;cursor:pointer;border:none}\
.sp-btn-primary{background:var(--primary);color:#fff}\
.sp-btn-primary:hover{background:var(--primary-hover)}\
.sp-btn-outline{background:transparent;color:var(--primary);border:1px solid var(--primary)}\
.sp-btn-outline:hover{background:var(--primary);color:#fff}\
.sp-hero{text-align:center;padding:80px 24px 60px;max-width:800px;margin:0 auto}\
.sp-hero h1{font-size:clamp(32px,5vw,52px);font-weight:800;letter-spacing:-1px;background:linear-gradient(135deg,var(--primary),#7c3aed);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}\
.sp-hero .sp-subtitle{font-size:18px;color:var(--text-secondary);margin-top:12px}\
.sp-hero .sp-desc{font-size:15px;color:var(--text-secondary);margin-top:16px;max-width:600px;margin-left:auto;margin-right:auto}\
.sp-hero-actions{margin-top:32px;display:flex;gap:12px;justify-content:center;flex-wrap:wrap}\
.sp-features{max-width:1100px;margin:0 auto;padding:40px 24px 60px}\
.sp-features h2{text-align:center;font-size:24px;font-weight:700;margin-bottom:32px}\
.sp-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:20px}\
.sp-card{background:var(--card-bg);border-radius:12px;padding:24px;border:1px solid var(--border);transition:box-shadow .2s,transform .2s}\
.sp-card:hover{box-shadow:var(--shadow);transform:translateY(-2px)}\
.sp-card-icon{width:40px;height:40px;border-radius:8px;background:linear-gradient(135deg,#e0f2fe,#dbeafe);display:flex;align-items:center;justify-content:center;font-size:20px;margin-bottom:12px}\
.sp-card h3{font-size:16px;font-weight:600;margin-bottom:6px}\
.sp-card p{font-size:13px;color:var(--text-secondary)}\
.sp-arch{max-width:900px;margin:0 auto;padding:20px 24px 60px;text-align:center}\
.sp-arch h2{font-size:24px;font-weight:700;margin-bottom:24px}\
.sp-arch-flow{display:flex;align-items:center;justify-content:center;gap:8px;flex-wrap:wrap;margin-bottom:24px}\
.sp-arch-node{padding:10px 18px;background:var(--card-bg);border:1px solid var(--border);border-radius:8px;font-size:13px;font-weight:600}\
.sp-arch-arrow{color:var(--text-secondary);font-size:18px}\
.sp-arch-layers{display:flex;gap:12px;justify-content:center;flex-wrap:wrap}\
.sp-arch-layer{padding:14px 20px;border-radius:8px;text-align:center;min-width:100px}\
.sp-footer{text-align:center;padding:32px 24px;color:var(--text-secondary);font-size:13px;border-top:1px solid var(--border)}\
@media(max-width:640px){.sp-header{padding:0 16px}.sp-nav{gap:12px}.sp-hero{padding:48px 16px 40px}.sp-grid{grid-template-columns:1fr}}\
</style>\
<header class="sp-header">\
  <div class="sp-logo">\
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/></svg>\
    Aetheris-MemOS\
  </div>\
  <nav class="sp-nav">\
    <a href="#/dashboard">Dashboard</a>\
    <a href="#/documentation">Documentation</a>\
    <a href="https://github.com/Colin4k1024/Aetheris-MemOS" target="_blank" rel="noopener">GitHub</a>\
  </nav>\
</header>\
<section class="sp-hero">\
  <h1>Aetheris-MemOS</h1>\
  <p class="sp-subtitle">Adaptive Memory Operating System for AI Agents</p>\
  <p class="sp-desc">\
    A multi-layer adaptive memory management system for AI Agent and LLM workloads.\
    Intelligent scheduling, decision tracing, and knowledge graph powered memory architecture.\
  </p>\
  <div class="sp-hero-actions">\
    <a class="sp-btn sp-btn-primary" href="#/documentation">Documentation</a>\
    <a class="sp-btn sp-btn-outline" href="#/dashboard">Dashboard</a>\
  </div>\
</section>\
<section class="sp-features">\
  <h2>Core Capabilities</h2>\
  <div class="sp-grid">\
    <div class="sp-card">\
      <div class="sp-card-icon">&#9889;</div>\
      <h3>Adaptive Memory Selection</h3>\
      <p>Intelligently selects optimal memory configuration based on task complexity, time range, and reasoning depth.</p>\
    </div>\
    <div class="sp-card">\
      <div class="sp-card-icon">&#127760;</div>\
      <h3>Multi-Layer Architecture</h3>\
      <p>STM, LTM, Knowledge Graph, and Multi-Modal memory layers working in coordination.</p>\
    </div>\
    <div class="sp-card">\
      <div class="sp-card-icon">&#128279;</div>\
      <h3>Decision Tracing</h3>\
      <p>Complete evidence graph with SHA-256 hash chain verification for full auditability.</p>\
    </div>\
    <div class="sp-card">\
      <div class="sp-card-icon">&#128200;</div>\
      <h3>Knowledge Graph Engine</h3>\
      <p>Entity-relationship modeling with graph traversal retrieval and GraphRAG hybrid search.</p>\
    </div>\
    <div class="sp-card">\
      <div class="sp-card-icon">&#127912;</div>\
      <h3>Multi-Modal Memory</h3>\
      <p>Unified storage and semantic retrieval for text, image, audio and other modalities.</p>\
    </div>\
    <div class="sp-card">\
      <div class="sp-card-icon">&#128640;</div>\
      <h3>Performance Optimization</h3>\
      <p>Dynamic weight adjustment, resource monitoring, and cost-benefit analysis for continuous improvement.</p>\
    </div>\
  </div>\
</section>\
<section class="sp-arch">\
  <h2>System Architecture</h2>\
  <div class="sp-arch-flow">\
    <div class="sp-arch-node">Client / Agent</div>\
    <span class="sp-arch-arrow">&rarr;</span>\
    <div class="sp-arch-node">API Gateway</div>\
    <span class="sp-arch-arrow">&rarr;</span>\
    <div class="sp-arch-node">Scheduler</div>\
    <span class="sp-arch-arrow">&rarr;</span>\
    <div class="sp-arch-node">Memory Layers</div>\
  </div>\
  <div class="sp-arch-layers">\
    <div class="sp-arch-layer" style="background:#e6f7ff;border:1px solid #91d5ff"><strong>STM</strong><br><small>Short-Term</small></div>\
    <div class="sp-arch-layer" style="background:#f6ffed;border:1px solid #b7eb8f"><strong>LTM</strong><br><small>Long-Term</small></div>\
    <div class="sp-arch-layer" style="background:#fff7e6;border:1px solid #ffd591"><strong>KG</strong><br><small>Knowledge Graph</small></div>\
    <div class="sp-arch-layer" style="background:#f9f0ff;border:1px solid #d3adf7"><strong>MM</strong><br><small>Multi-Modal</small></div>\
  </div>\
</section>\
<footer class="sp-footer">\
  <p>Aetheris-MemOS &mdash; Built with Rust (Axum) + React (Ant Design Pro)</p>\
  <p style="margin-top:4px"><a href="https://github.com/Colin4k1024/Aetheris-MemOS" style="color:var(--primary);text-decoration:none">GitHub</a> &middot; MIT License</p>\
</footer>';
  }
})();
