// AI Monopoly arena — polls /ai-monopoly/live and draws the conversation as it lands.
// Turns arrive whole (the worker posts one per completed generation); the typewriter is
// client-side, so message-level polling reads as live without any streaming plumbing.
// Same heartbeat shape as harness.js: tiny JSON, no framework.
(function () {
  var arena = document.getElementById('arena');
  if (!arena || arena.dataset.live !== 'true') return;

  var POLL_MS = 2000;
  var TYPE_MS = 12;          // per character
  var seq = 0;               // highest turn already drawn for the focused match
  var drawn = null;          // match id currently on screen
  var pinned = null;         // match the student clicked; null = follow whatever is live
  var queue = [];            // bubbles waiting to be typed, so they never overlap
  var typing = false;
  var cash = {};             // player id -> last drawn cash, for the tick animation

  function money(v) { return (v || 0).toLocaleString('tr-TR') + ' ₺'; }
  function el(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;   // textContent, never innerHTML: this is model output
    return n;
  }

  // ---- typewriter -----------------------------------------------------------
  function typeNext() {
    if (typing || !queue.length) return;
    typing = true;
    var job = queue.shift();
    var i = 0;
    var t = setInterval(function () {
      job.node.textContent = job.text.slice(0, ++i);
      job.scroll();
      if (i >= job.text.length) {
        clearInterval(t);
        job.node.parentNode.classList.remove('typing');
        typing = false;
        typeNext();
      }
    }, TYPE_MS);
  }

  // ---- rendering ------------------------------------------------------------
  function fighter(side, name, s) {
    var box = el('div', 'fighter ' + side);
    box.appendChild(el('div', 'ava', (name || '?').charAt(0).toUpperCase()));
    box.appendChild(el('div', 'fname', name));
    box.appendChild(el('div', 'fprod', s ? s.product_name : ''));
    var c = el('div', 'fcash', money(s ? s.cash : 0));
    if (s) {
      if (cash[s.id] != null && cash[s.id] !== s.cash) {
        c.classList.add(s.cash > cash[s.id] ? 'up' : 'down');
      }
      cash[s.id] = s.cash;
      box.appendChild(c);
      box.appendChild(el('div', 'fgoods', money(s.goods) + ' mal'));
    } else {
      box.appendChild(c);
    }
    return box;
  }

  function drawStrip(d) {
    var strip = el('div', 'arena-strip');
    strip.appendChild(el('span', 'striplabel', 'Tur ' + d.round + '/' + d.rounds_total));
    d.matches.forEach(function (m) {
      var cls = m.status === 'done' ? 'done' : (m.status === 'running' ? 'live' : 'wait');
      var b = el('button', 'ms ' + cls + ' k-' + m.kind + (d.match && m.id === d.match.id ? ' on' : ''),
                 m.a_name + ' ↔ ' + m.b_name);
      b.title = m.kind === 'chosen' ? 'Davet — modelin kendi seçtiği konuşma' : 'Eşleşme';
      b.onclick = function () { pinned = m.id; refresh(true); };
      strip.appendChild(b);
    });
    return strip;
  }

  function drawVerdict(m) {
    var box = el('div', 'arena-verdict');
    if (!m.transactions.length) {
      box.appendChild(el('p', 'muted', 'Hakem: satış yok — anlaşma çıkmadı.'));
      return box;
    }
    box.appendChild(el('h3', null, 'Hakem kararı'));
    m.transactions.forEach(function (t) {
      var row = el('div', 'vrow');
      row.appendChild(el('span', 'vflow', t.seller + ' → ' + t.buyer));
      row.appendChild(el('span', 'vitem', t.item));
      row.appendChild(el('span', 'vprice', money(t.price)));
      // surplus is the lesson: paying more than the thing is worth to you shows red
      row.appendChild(el('span', 'vsurp ' + (t.surplus >= 0 ? 'ok' : 'bad'),
                         (t.surplus >= 0 ? '+' : '') + money(t.surplus)));
      box.appendChild(row);
      if (t.reasoning) box.appendChild(el('p', 'vwhy', t.reasoning));
    });
    return box;
  }

  function byId(standings) {
    var map = {};
    standings.forEach(function (s) { map[s.id] = s; });
    return map;
  }

  function draw(d) {
    var m = d.match;
    var s = byId(d.standings);
    var fresh = !m || m.id !== drawn;

    if (fresh) {
      arena.textContent = '';
      queue = [];
      seq = 0;
      drawn = m ? m.id : null;
    } else {
      // header, strip and verdict are cheap to rebuild; the chat is not, so it stays
      ['arena-strip', 'arena-invites', 'arena-head', 'arena-verdict'].forEach(function (c) {
        var n = arena.querySelector('.' + c);
        if (n) n.remove();
      });
    }

    var chat = arena.querySelector('.arena-chat');
    if (!chat) { chat = el('div', 'arena-chat'); }
    else { chat.remove(); }

    arena.appendChild(drawStrip(d));
    if (d.invites && d.invites.length) {
      var inv = el('p', 'arena-invites');
      inv.textContent = d.invites.map(function (i) {
        return i.from + (i.accepted ? ' → ' : ' ⊘ ') + i.to;
      }).join('   ·   ');
      inv.title = 'Modellerin birbirine yaptığı davetler — ⊘ reddedildi';
      arena.appendChild(inv);
    }
    if (!m) {
      arena.appendChild(el('p', 'muted', d.progress || 'Eşleşme bekleniyor…'));
      return;
    }

    var turns = { a: 0, b: 0 };
    chat.querySelectorAll('.bub').forEach(function (b) { turns[b.dataset.side]++; });
    m.messages.forEach(function (x) { turns[x.speaker]++; });

    var head = el('div', 'arena-head');
    head.appendChild(fighter('left', m.a_name, s[m.a_id]));
    var vs = el('div', 'vs');
    vs.appendChild(el('div', 'vsround', m.kind === 'chosen' ? 'Davet' : 'Eşleşme'));
    vs.appendChild(el('div', 'vsturns', turns.a + ' — ' + turns.b));
    head.appendChild(vs);
    head.appendChild(fighter('right', m.b_name, s[m.b_id]));
    arena.appendChild(head);
    arena.appendChild(chat);

    m.messages.forEach(function (x) {
      if (x.seq <= seq) return;
      seq = x.seq;
      var wrap = el('div', 'bub ' + (x.speaker === 'a' ? 'l' : 'r') + ' typing');
      wrap.dataset.side = x.speaker;
      wrap.appendChild(el('div', 'who', x.speaker === 'a' ? m.a_name : m.b_name));
      var body = el('div', 'say');
      wrap.appendChild(body);
      chat.appendChild(wrap);
      queue.push({
        node: body, text: x.content,
        scroll: function () { chat.scrollTop = chat.scrollHeight; },
      });
    });
    typeNext();

    if (m.status === 'done') {
      if (!chat.querySelector('.ended')) chat.appendChild(el('div', 'ended', 'Konuşma bitti'));
      arena.appendChild(drawVerdict(m));
    } else if (m.status === 'failed') {
      arena.appendChild(el('p', 'muted', 'Bu konuşma yarıda kaldı.'));
    }
  }

  function drawStandings(rows) {
    var box = document.getElementById('arena-standings');
    if (!box) return;
    box.textContent = '';
    rows.forEach(function (r, i) {
      var row = el('div', 'lbrow ' + (i < 3 ? 'm' + (i + 1) : ''));
      row.appendChild(el('span', 'lbrank', String(i + 1)));
      var name = el('span', 'lbname', r.char_name);
      name.appendChild(el('span', 'lbmeta', r.team_name + ' · ' + r.product_name));
      row.appendChild(name);
      var pts = el('span', 'lbpts', money(r.net));
      pts.appendChild(el('span', 'lbmeta', money(r.cash) + ' nakit · ' + money(r.goods) + ' mal'));
      row.appendChild(pts);
      box.appendChild(row);
    });
  }

  // ---- polling --------------------------------------------------------------
  var timer = null;
  function refresh(now) {
    // `have` tells the server which conversation `seq` counts turns of, so a switch of
    // focus replays the new one from the start instead of silently returning nothing
    var url = '/ai-monopoly/live?seq=' + (now ? 0 : seq)
      + (drawn && !now ? '&have=' + drawn : '')
      + (pinned ? '&match=' + pinned : '');
    fetch(url)
      .then(function (r) { return r.json(); })
      .then(function (d) {
        if (!d.status) return;
        // terminal: one reload swaps in the server-rendered end state and stops the poll
        if (d.status === 'done' || d.status === 'failed') {
          clearInterval(timer);
          location.reload();
          return;
        }
        if (now) { drawn = null; seq = 0; }
        draw(d);
        drawStandings(d.standings);
      })
      .catch(function () {});   // transient network error — next tick retries
  }

  refresh(true);
  timer = setInterval(refresh, POLL_MS);
})();

// History replay — the transcript ships in both languages and the toggle flips one class,
// so switching is instant and the English original is never more than a click away.
(function () {
  var replay = document.getElementById('replay');
  if (!replay) return;
  document.querySelectorAll('.langtoggle .chip').forEach(function (b) {
    b.onclick = function () {
      document.querySelectorAll('.langtoggle .chip').forEach(function (o) {
        o.classList.toggle('active', o === b);
      });
      replay.classList.toggle('show-tr', b.dataset.lang === 'tr');
    };
  });
})();
