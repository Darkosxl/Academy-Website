// ARC-AGI-3 viewer: 25 pixel boards polled live, click one to watch it big at ~4fps.
// Frames are 4096-char hex grids; canvases stay 64x64 and CSS scales them, so no maths here.
(function () {
  'use strict';

  var root = document.getElementById('arc-live');
  if (!root || root.dataset.active !== 'true') return;

  // 2s matches harness.js. The main tab's preview sets data-poll="5000": every poll
  // returns all 25 grids and the preview only shows one, so it does not need the pace.
  var POLL_MS = Number(root.dataset.poll) || 2000;
  var IDLE_MS = 15000;  // nothing running: slow enough to be free, fast enough to catch a new run
  var FRAME_MS = 250;   // ~4fps, so an action's animation reads as motion between polls
  var PAGE = 200;       // the endpoint's focus page size — a shorter page means we caught up
  var SIDE = 64;        // arcengine MAX_DIMENSION: every grid is exactly 64x64
  var CELLS = SIDE * SIDE;
  var TERMINAL = ['done', 'partial', 'failed', 'infra_failed', 'cancelled'];
  var STATE_TR = {
    NOT_PLAYED: 'Başlamadı',
    NOT_FINISHED: 'Oynanıyor',
    WIN: 'Kazandı',
    GAME_OVER: 'Bitti'
  };
  var RANK = {NOT_FINISHED: 0, WIN: 1, GAME_OVER: 1, NOT_PLAYED: 2};   // rail order
  // the engine's canonical 16-colour map, alpha dropped
  var PALETTE = ['#FFFFFF', '#CCCCCC', '#999999', '#666666', '#333333', '#000000', '#E53AA3',
                 '#FF7BCC', '#F93C31', '#1E93FF', '#88D8F1', '#FFDC00', '#FF851B', '#921231',
                 '#4FCC30', '#A356D6'];
  var RGB = [];         // flat [r,g,b, r,g,b, ...] — parsed once, not per pixel
  for (var c = 0; c < PALETTE.length; c++) {
    RGB.push(parseInt(PALETTE[c].slice(1, 3), 16),
             parseInt(PALETTE[c].slice(3, 5), 16),
             parseInt(PALETTE[c].slice(5, 7), 16));
  }

  var boards = document.getElementById('arc-boards');
  var focusBox = document.getElementById('arc-focus');
  var idle = document.getElementById('arc-idle');

  // The main tab has no board wall — just these three, mirrored from the focused game.
  var prevCanvas = document.getElementById('arc-preview');
  var prevMeta = document.getElementById('arc-preview-meta');
  var prevGame = document.getElementById('arc-preview-game');
  var counters = document.querySelectorAll('[data-arc-count]');

  var replay = root.dataset.replay === 'true';
  var runId = root.dataset.run || '';
  var currentRun = root.dataset.currentRun || '';
  var tiles = {};        // game id -> {button, canvas, meta}
  var focused = null;    // game the student clicked; null = grid only, no focus= param
  var film = [];         // focused game's animation, flattened to one entry per grid
  var at = -1;           // index of the grid currently on the big canvas
  var bufSeq = null;     // highest frame seq buffered — what the next poll asks past
  var paused = false;    // the student took the scrubber over
  var tailing = true;    // following a run in flight, so falling behind is worth skipping
  var wasLive = false;   // saw a non-terminal stage: a terminal one is now a transition
  var picked = false;    // auto-focus has run once; a student's click owns it from here
  var big = null, bigMeta = null, scrub = null, playBtn = null, timer = null, pace = 0;

  function el(tag, cls, text) {
    var node = document.createElement(tag);
    if (cls) node.className = cls;
    if (text != null) node.textContent = text;   // textContent, never innerHTML: DB output
    return node;
  }

  // ---- drawing --------------------------------------------------------------
  var shared = null;   // one ImageData for every canvas — they are all 64x64

  function draw(canvas, hex) {
    if (!canvas || typeof hex !== 'string' || hex.length < CELLS) return;
    var ctx = canvas.getContext('2d');
    if (!shared) shared = ctx.createImageData(SIDE, SIDE);
    var data = shared.data;
    for (var i = 0; i < CELLS; i++) {
      var value = parseInt(hex.charAt(i), 16);
      var p = (value >= 0 ? value : 5) * 3;   // unreadable cell -> #000000
      var o = i * 4;
      data[o] = RGB[p];
      data[o + 1] = RGB[p + 1];
      data[o + 2] = RGB[p + 2];
      data[o + 3] = 255;
    }
    ctx.putImageData(shared, 0, 0);
  }

  // ---- the grid of 25 -------------------------------------------------------
  function tileFor(game) {
    if (tiles[game]) return tiles[game];
    var button = el('button', 'arc-tile');
    button.type = 'button';
    button.dataset.game = game;
    button.setAttribute('aria-pressed', 'false');
    var canvas = document.createElement('canvas');
    canvas.width = SIDE;
    canvas.height = SIDE;
    canvas.setAttribute('role', 'img');
    canvas.setAttribute('aria-label', game + ' oyun tahtası');
    button.appendChild(canvas);
    button.appendChild(el('span', 'arc-name', game));
    var meta = el('span', 'arc-meta', '');
    button.appendChild(meta);
    button.onclick = function () { focusGame(game); };
    boards.appendChild(button);
    tiles[game] = {button: button, canvas: canvas, meta: meta};
    return tiles[game];
  }

  function total(values) {
    var out = 0;
    if (values) for (var i = 0; i < values.length; i++) out += values[i] || 0;
    return out;
  }

  // Actions used against the human baseline is the number this whole view exists to
  // show, so it rides on every tile: "Seviye 2 · 142/218 hamle · Oynanıyor".
  function line(game) {
    var human = total(game.baseline);
    var used = game.total || 0;
    return ['Seviye ' + (game.levels_completed || 0),
            human ? used + '/' + human + ' hamle' : used + ' hamle',
            STATE_TR[game.state] || game.state || ''].join(' · ');
  }

  // "5 oynuyor · 8 bitti / 25" on the main tab's preview card.
  function counts(playing, done, all) {
    for (var i = 0; i < counters.length; i++) {
      var node = counters[i];
      var key = node.dataset.arcCount;
      node.textContent = key === 'playing' ? String(playing)
        : key === 'done' ? String(done)
        : key === 'total' ? String(all)
        : playing + ' oyun oynanıyor';
    }
  }

  // Nothing is focused until something is clicked, but the main tab's preview has no
  // tile to click and the watch page should be watching the moment it opens. Runs once:
  // `focused` is set from here on, and focusGame early-returns on a repeat, so a student
  // clicking another board always wins.
  function autoFocus(games) {
    if (picked || focused) return;
    picked = true;
    var pick = games[0].game;
    for (var i = 0; i < games.length; i++) {
      if (games[i].state === 'NOT_FINISHED') { pick = games[i].game; break; }
    }
    focusGame(pick);
  }

  function renderGames(games) {
    if (!games || !games.length) return;
    if (idle) idle.hidden = true;
    var playing = 0, done = 0;
    for (var i = 0; i < games.length; i++) {
      var game = games[i];
      if (game.state === 'NOT_FINISHED') playing++;
      else if (game.state === 'WIN' || game.state === 'GAME_OVER') done++;
      if (!boards) continue;   // main-tab preview: focus panel only, no board wall
      var tile = tileFor(game.game);
      tile.button.dataset.state = game.state || '';
      tile.meta.textContent = line(game);
      // The five that are actually playing lead the column, then the finished ones,
      // then the queue. `order` on a grid item, so nothing is moved in the DOM and
      // equal ranks keep ARC_GAMES order.
      tile.button.style.order = RANK[game.state] != null ? RANK[game.state] : 3;
      draw(tile.canvas, game.grid);
    }
    counts(playing, done, games.length);
    autoFocus(games);
  }

  // ---- the focused board ----------------------------------------------------
  // `paused` used to be a one-way door: the scrubber sets it and nothing clears it
  // except focusing a *different* game, so scrubbing once froze the board for good.
  function setPaused(value) {
    paused = value;
    if (!playBtn) return;
    playBtn.textContent = paused ? '▶' : '❚❚';
    playBtn.setAttribute('aria-label', paused ? 'Oynat' : 'Duraklat');
  }

  function buildFocus() {
    big = document.createElement('canvas');
    big.width = SIDE;
    big.height = SIDE;
    big.setAttribute('role', 'img');
    focusBox.appendChild(big);
    bigMeta = el('p', 'arc-focus-meta', '');
    focusBox.appendChild(bigMeta);
    playBtn = el('button', 'arc-play');
    playBtn.type = 'button';
    playBtn.onclick = function () { setPaused(!paused); };
    focusBox.appendChild(playBtn);
    setPaused(false);
  }

  function focusGame(game) {
    if (focused === game) return;
    focused = game;
    picked = true;
    film = [];
    at = -1;
    bufSeq = null;
    // the player only earns the two-column watch layout once there is a board wall
    // beside it to be the playlist
    if (boards) root.classList.add('watching');
    if (scrub) { scrub.parentNode.removeChild(scrub); scrub = null; }
    for (var key in tiles) {
      if (Object.prototype.hasOwnProperty.call(tiles, key)) {
        tiles[key].button.setAttribute('aria-pressed', key === game ? 'true' : 'false');
      }
    }
    focusBox.hidden = false;
    if (!big) buildFocus();
    setPaused(false);
    if (prevGame) prevGame.textContent = game;
    // Blank the board before the first frame of the new game lands. Without this, a failed
    // or slow fetch leaves the PREVIOUS game's board on screen under the new game's label.
    big.getContext('2d').clearRect(0, 0, SIDE, SIDE);
    bigMeta.textContent = game + ' · yükleniyor…';
    big.setAttribute('aria-label', game + ' oyun tahtası, büyük görünüm');
    bigMeta.textContent = game;
    start();
    tick();
  }

  function caption(game, frame) {
    var parts = [game, '#' + frame.seq];
    if (frame.action) {
      parts.push(frame.action + (frame.action_x != null
        ? ' (' + frame.action_x + ',' + frame.action_y + ')' : ''));
    }
    parts.push('Seviye ' + (frame.levels_completed || 0));
    parts.push(STATE_TR[frame.state] || frame.state || '');
    return parts.join(' · ');
  }

  function buffer(payload) {
    var frames = payload.frames || [];
    for (var i = 0; i < frames.length; i++) {
      var frame = frames[i];
      if (bufSeq != null && frame.seq <= bufSeq) continue;
      bufSeq = frame.seq;
      var text = caption(payload.game, frame);
      var grids = frame.grids || [];
      for (var j = 0; j < grids.length; j++) film.push({hex: grids[j], text: text});
    }
    return frames.length;
  }

  function show() {
    var cell = film[at];
    if (!cell) return;
    draw(big, cell.hex);
    bigMeta.textContent = cell.text;
    if (prevCanvas) draw(prevCanvas, cell.hex);
    if (prevMeta) prevMeta.textContent = cell.text;
    if (scrub && document.activeElement !== scrub) scrub.value = String(at);
  }

  function play() {
    if (paused || at >= film.length - 1) return;
    // a death animation dumps up to 16 grids at once; while a run is in flight the point
    // is to be near the live board, so drop the backlog instead of drifting minutes behind
    if (tailing && film.length - at > 40) at = film.length - 40;
    at++;
    show();
  }

  function scrubber() {
    if (scrub || film.length < 2) return;
    scrub = document.createElement('input');
    scrub.type = 'range';
    scrub.min = '0';
    scrub.step = '1';
    scrub.value = String(at < 0 ? 0 : at);
    scrub.setAttribute('aria-label', 'Kare seçici');
    scrub.oninput = function () {
      setPaused(true);   // the student is steering now; playback stops chasing them
      at = Number(scrub.value);
      show();
    };
    focusBox.appendChild(scrub);
  }

  // ---- polling --------------------------------------------------------------
  function url() {
    var query = [];
    if (runId) query.push('run=' + encodeURIComponent(runId));
    if (focused) {
      query.push('game=' + encodeURIComponent(focused));
      // no seq on the first request for a game: absent means "from frame 0"
      if (bufSeq != null) query.push('seq=' + bufSeq);
    }
    return '/agentic-harness/arc/live' + (query.length ? '?' + query.join('&') : '');
  }

  function apply(payload) {
    if (!payload) return;
    var nextRun = payload.run || '';
    if (!replay && nextRun !== currentRun) {
      stop();
      location.reload();
      return;
    }
    var over = !!payload.run && TERMINAL.indexOf(payload.stage) !== -1;
    if (!over) {
      wasLive = true;
    } else if (wasLive && !replay) {
      // the run finished while watching: one reload swaps in the server-rendered end
      // state, so the scores and the history stay canonical — same rule as harness.js
      stop();
      location.reload();
      return;
    }
    tailing = !over && !replay;
    if (tailing) start(POLL_MS);   // a run (re)started: back up to the live heartbeat
    renderGames(payload.games);

    var focus = payload.focus;
    if (focus && focus.game === focused) {
      var got = buffer(focus);
      if (at < 0 && film.length) { at = 0; show(); }
      if (scrub) scrub.max = String(film.length - 1);
      // a short page is the end of the run's frames for this game: stop polling and
      // hand the student the scrubber over everything buffered
      if (!tailing && got < PAGE) {
        stop();
        scrubber();
        if (scrub) scrub.max = String(film.length - 1);
      }
    } else if (!tailing && !focused) {
      // Finished run, nothing focused: these boards are final. But in live mode the student
      // may submit again while sitting here, so idle down instead of stopping — a hard stop
      // leaves the tab permanently blank for every later run until a manual reload.
      if (replay) stop(); else start(IDLE_MS);
    }
  }

  function tick() {
    fetch(url(), {headers: {'Accept': 'application/json'}})
      .then(function (response) {
        if (!response.ok) throw new Error('status ' + response.status);
        return response.json();
      })
      .then(apply)
      .catch(function () {});
  }

  function start(ms) {
    var want = ms || POLL_MS;
    if (timer && pace === want) return;
    stop();
    pace = want;
    timer = setInterval(tick, want);
  }
  function stop() { if (timer) { clearInterval(timer); timer = null; pace = 0; } }

  start();
  tick();
  setInterval(play, FRAME_MS);
})();
