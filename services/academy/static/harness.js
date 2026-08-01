// Live Agentic Harness cards. Each benchmark is independent; terminal run
// states trigger one reload so the form/history and partial scores stay canonical.
(function () {
  'use strict';

  var live = document.getElementById('harness-live');
  if (!live || live.dataset.active !== 'true') return;

  var TERMINAL = ['done', 'partial', 'failed', 'infra_failed', 'cancelled'];
  var STATUS = {
    pending: ['Bekliyor', 'st-pending'],
    running: ['Çalışıyor', 'st-reviewing'],
    done: ['Tamamlandı', 'st-passed'],
    failed: ['Başarısız', 'st-failed'],
    infra_failed: ['Altyapı hatası', 'st-failed'],
    cancelled: ['Durduruldu', 'st-failed']
  };
  var RUN_STATUS = {
    queued: ['Sırada', 'st-pending'],
    preparing: ['Repo hazırlanıyor', 'st-reviewing'],
    running: ["Benchmark'lar çalışıyor", 'st-reviewing']
  };
  var countdown = document.getElementById('harness-countdown');

  function number(value, suffix) {
    return typeof value === 'number' && isFinite(value) ? value.toFixed(1) + (suffix || '') : null;
  }

  function renderBenchmark(key, state) {
    var card = live.querySelector('[data-benchmark="' + key + '"]');
    if (!card) return;
    state = state && typeof state === 'object' ? state : {status: 'pending'};
    var status = STATUS[state.status] || STATUS.pending;
    var pill = card.querySelector('.benchmark-status');
    pill.textContent = status[0];
    pill.className = 'substatus benchmark-status ' + status[1];
    card.dataset.status = state.status || 'pending';

    var parts = [];
    if (state.done != null && state.total != null) parts.push(state.done + '/' + state.total);
    var score = number(state.score);
    if (score) parts.push('Skor ' + score);
    if (key === 'ram') {
      var one = number(state.one_session_mb, ' MB');
      var ten = number(state.ten_session_mb, ' MB');
      if (one) parts.push('1 oturum ' + one);
      if (ten) parts.push('10 oturum ' + ten);
    }
    if (state.active != null) parts.push(state.active + ' aktif');
    if (state.rate != null) {
      var rate = state.rate + ' tur / 30 sn';
      if (state.rate_target != null) rate += ' (hedef ' + state.rate_target + ')';
      parts.push(rate);
    }
    if (state.error) parts.push(String(state.error).slice(0, 300));
    card.querySelector('.benchmark-progress').textContent = parts.join(' · ') || 'Henüz sonuç yok';
  }

  function renderRunStatus(stage) {
    var pill = document.getElementById('harness-run-status');
    var status = RUN_STATUS[stage];
    if (!pill || !status) return;
    pill.textContent = status[0];
    pill.className = 'substatus ' + status[1];
  }

  function apply(payload) {
    if (!payload || payload.stage == null || TERMINAL.indexOf(payload.stage) !== -1) {
      location.reload();
      return;
    }
    if (payload.run && payload.run !== live.dataset.run) {
      location.reload();
      return;
    }
    renderRunStatus(payload.stage);
    var states = payload.benchmarks || {};
    ['arc', 'frontier', 'ram'].forEach(function (key) {
      renderBenchmark(key, states[key]);
    });
    var commit = document.getElementById('harness-commit');
    if (commit && payload.commit_sha) commit.textContent = payload.commit_sha.slice(0, 7);
    var profile = document.getElementById('harness-profile');
    if (profile && payload.bedrock_profile) profile.textContent = payload.bedrock_profile;
    if (payload.deadline_at) live.dataset.deadline = payload.deadline_at;
  }

  function renderCountdown() {
    if (!countdown || !live.dataset.deadline) return;
    var remaining = Math.max(0, Math.ceil((Date.parse(live.dataset.deadline) - Date.now()) / 1000));
    var minutes = Math.floor(remaining / 60);
    countdown.textContent = 'Sert sınır ' + minutes + ':' + String(remaining % 60).padStart(2, '0');
  }

  function tick() {
    fetch('/agentic-harness/status', {headers: {'Accept': 'application/json'}})
      .then(function (response) {
        if (!response.ok) throw new Error('status ' + response.status);
        return response.json();
      })
      .then(apply)
      .catch(function () {});
  }

  renderCountdown();
  tick();
  setInterval(renderCountdown, 1000);
  setInterval(tick, 2000);
})();
