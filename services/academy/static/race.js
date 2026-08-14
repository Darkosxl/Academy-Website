// AI Monopoly horse race. Polls team win counts from the Academy and slides each
// team's jockey lane left-to-right toward the finish line.
(function () {
  'use strict';

  var root = document.getElementById('horse-race');
  if (!root) return;

  var total = parseInt(root.dataset.total, 10) || 1;

  function render(teams) {
    teams.forEach(function (team) {
      var lane = root.querySelector('[data-entry="' + team.entry_id + '"]');
      if (!lane) return;
      var pct = Math.max(0, Math.min(100, (team.wins / total) * 100));
      var video = lane.querySelector('.horse-video');
      if (video) video.style.left = 'calc(' + pct + '% - ' + pct / 100 * 64 + 'px)';
      var wins = lane.querySelector('.horse-wins');
      if (wins) wins.textContent = team.wins + ' galibiyet';
    });
  }

  function refresh() {
    fetch('/ai-monopoly/race.json', {headers: {'Accept': 'application/json'}})
      .then(function (response) { return response.json(); })
      .then(function (data) { if (data.teams) render(data.teams); })
      .catch(function () {});
  }

  refresh();
  setInterval(refresh, 2000);
})();
