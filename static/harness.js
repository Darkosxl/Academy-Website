// Agentic Harness — while the team has a run in flight, poll its status and move
// the stepper; one reload on a terminal state brings back the form and the scores.
// Mirrors tracker.js's heartbeat approach: tiny JSON, no framework.
(function () {
  var stepper = document.getElementById('harness-stepper');
  if (!stepper || stepper.dataset.active !== 'true') return;
  var STAGES = ['queued', 'cloning', 'building', 'arc_agi_3', 'frontier_bench', 'ram_bench'];

  function apply(stage) {
    if (stage === null || stage === 'done' || stage === 'failed') {
      location.reload();
      return;
    }
    var cur = STAGES.indexOf(stage);
    if (cur < 0) return;
    stepper.querySelectorAll('.step').forEach(function (li, i) {
      li.classList.toggle('done', i < cur);
      li.classList.toggle('active', i === cur);
    });
  }

  function tick() {
    fetch('/agentic-harness/status')
      .then(function (r) { return r.json(); })
      .then(function (s) { apply(s.stage); })
      .catch(function () {}); // transient network error — next tick retries
  }

  setInterval(tick, 5000);
})();
