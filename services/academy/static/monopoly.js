// AI Monopoly board renderer. Live pages cursor-poll the Academy event log; history
// pages feed the same renderer from an inert JSON script element.
(function () {
  'use strict';

  var arena = document.getElementById('monopoly-arena');
  if (!arena) return;

  var BOARD = [
    'GO', 'Mediterranean Ave', 'Community Chest', 'Baltic Ave', 'Income Tax',
    'Reading Railroad', 'Oriental Ave', 'Chance', 'Vermont Ave', 'Connecticut Ave',
    'Jail / Visiting', 'St. Charles Place', 'Electric Company', 'States Ave', 'Virginia Ave',
    'Pennsylvania Railroad', 'St. James Place', 'Community Chest', 'Tennessee Ave', 'New York Ave',
    'Free Parking', 'Kentucky Ave', 'Chance', 'Indiana Ave', 'Illinois Ave',
    'B&O Railroad', 'Atlantic Ave', 'Ventnor Ave', 'Water Works', 'Marvin Gardens',
    'Go to Jail', 'Pacific Ave', 'North Carolina Ave', 'Community Chest', 'Pennsylvania Ave',
    'Short Line Railroad', 'Chance', 'Park Place', 'Luxury Tax', 'Boardwalk'
  ];
  var COLORS = {};
  [[1,3], [6,8,9], [11,13,14], [16,18,19], [21,23,24], [26,27,29], [31,32,34], [37,39]]
    .forEach(function (squares, i) {
      var names = ['brown','lightblue','pink','orange','red','yellow','green','darkblue'];
      squares.forEach(function (square) { COLORS[square] = names[i]; });
    });
  [5,15,25,35].forEach(function (square) { COLORS[square] = 'railroad'; });
  [12,28].forEach(function (square) { COLORS[square] = 'utility'; });

  var STATUS = {
    queued: ['Sırada', 'st-pending'],
    leased: ['Oynanıyor', 'st-reviewing'],
    done: ['Tamamlandı', 'st-passed'],
    failed: ['Altyapı hatası', 'st-failed'],
    cancelled: ['Durduruldu', 'st-failed']
  };
  var PHASE = {
    pre_roll: 'Zar öncesi', post_roll: 'Zar sonrası',
    out_of_turn: 'Sıra dışı karar', auction: 'Açık artırma'
  };

  var seq = 0;
  var pinnedGameId = arena.dataset.gameId || null;
  var replayMode = arena.dataset.replay === 'true';
  var gameId = pinnedGameId;
  var attempt = null;
  var events = [];
  var lastSnapshot = null;
  var timer = null;
  var initialRequest = true;
  var replaySeq = null;
  var replayAvailable = false;
  var replayTimer = null;
  var latestPayload = null;

  function el(tag, cls, text) {
    var node = document.createElement(tag);
    if (cls) node.className = cls;
    if (text != null) node.textContent = text;
    return node;
  }

  function money(value) {
    var amount = typeof value === 'number' && isFinite(value) ? Math.round(value) : 0;
    return '$' + amount.toLocaleString('en-US');
  }

  function coords(square) {
    if (square <= 10) return [11, 11 - square];
    if (square <= 20) return [21 - square, 1];
    if (square <= 30) return [1, square - 19];
    return [square - 29, 11];
  }

  function seatById(payload, id) {
    return (payload.seats || []).find(function (seat) { return seat.player_id === id; });
  }

  function propertyBySquare(snapshot) {
    var map = {};
    (snapshot && snapshot.properties || []).forEach(function (property) {
      map[property.square_id] = property;
    });
    return map;
  }

  function actionText(action) {
    var direct = {
      DO_NOTHING: 'Bekledi', END_TURN: 'Sırayı bitirdi', ROLL_DICE: 'Zar attı',
      BUY_PROPERTY: 'Mülkü satın aldı', USE_GOOJ_CARD: 'Hapisten çıkış kartını kullandı',
      PAY_BAIL: 'Kefalet ödedi', DECLARE_BANKRUPT: 'İflas etti',
      ACCEPT_TRADE: 'Takası kabul etti', DECLINE_TRADE: 'Takası reddetti',
      auction_pass: 'Açık artırmayı pas geçti'
    };
    if (direct[action]) return direct[action];
    var square = action && action.match(/sq=(\d+)/);
    var place = square ? BOARD[Number(square[1])] : '';
    if (action.indexOf('mortgage(') === 0) return place + ' ipotek edildi';
    if (action.indexOf('unmortgage(') === 0) return place + ' ipoteği kaldırıldı';
    if (action.indexOf('improve_house(') === 0) return place + ' üzerine ev yaptı';
    if (action.indexOf('improve_hotel(') === 0) return place + ' üzerine otel yaptı';
    if (action.indexOf('sell_house(') === 0) return place + ' evini sattı';
    if (action.indexOf('sell_hotel(') === 0) return place + ' otelini sattı';
    if (action.indexOf('sell_prop(') === 0) return place + ' mülkünü bankaya sattı';
    if (action.indexOf('auction_bid(') === 0) return 'Açık artırmada ' + action.slice(12, -1) + ' artırdı';
    if (action.indexOf('buy_trade(') === 0) return 'Satın alma takası teklif etti';
    if (action.indexOf('sell_trade(') === 0) return 'Satış takası teklif etti';
    if (action.indexOf('exch_trade(') === 0) return 'Mülk takası teklif etti';
    return String(action || 'Karar verdi').replaceAll('_', ' ');
  }

  function playerCards(payload, snapshot) {
    var strip = el('div', 'monopoly-players');
    (payload.seats || []).forEach(function (seat) {
      var player = (snapshot && snapshot.players || []).find(function (row) {
        return row.player_id === seat.player_id;
      });
      var card = el('section', 'monopoly-player p' + seat.player_id);
      if (snapshot && snapshot.active_player === seat.player_id) card.classList.add('active');
      if (payload.winner_seat === seat.player_id &&
          (!replayMode || replaySeq == null || replaySeq >= Number(payload.action_count))) {
        card.classList.add('winner');
      }
      if (player && player.bankrupt) card.classList.add('bankrupt');
      var head = el('div', 'monopoly-player-head');
      head.appendChild(el('span', 'player-token', String(seat.player_id + 1)));
      head.appendChild(el('b', null, seat.label));
      card.appendChild(head);
      if (player) {
        card.appendChild(el('strong', 'player-worth', money(player.net_worth)));
        card.appendChild(el('span', 'player-meta', money(player.cash) + ' nakit · ' +
          (player.properties || []).length + ' tapu'));
        if (player.in_jail) card.appendChild(el('span', 'player-flag', 'Hapiste'));
        if (player.bankrupt) card.appendChild(el('span', 'player-flag', 'İflas'));
      } else {
        if (seat.final_net_worth != null) {
          card.appendChild(el('strong', 'player-worth', money(seat.final_net_worth)));
          card.appendChild(el('span', 'player-meta', money(seat.final_cash) + ' final nakit'));
        } else {
          card.appendChild(el('span', 'player-meta', 'İlk hamle bekleniyor'));
        }
      }
      if (seat.entry_id) {
        var calls = Number(seat.decision_count || 0);
        var mean = calls ? Number(seat.decision_total_us || 0) / calls / 1000 : 0;
        var timing = calls
          ? 'Ort ' + mean.toFixed(1) + ' ms · ' +
            (Number(seat.decision_min_us || 0) / 1000).toFixed(1) + '–' +
            (Number(seat.decision_max_us || 0) / 1000).toFixed(1) + ' ms'
          : 'Karar bekleniyor';
        card.appendChild(el('span', 'player-meta player-timing', timing));
        if (seat.strikes) card.appendChild(el('span', 'player-flag', seat.strikes + ' strike'));
        if (seat.disqualified) card.appendChild(el('span', 'player-flag', 'Diskalifiye'));
      }
      strip.appendChild(card);
    });
    return strip;
  }

  function board(payload, snapshot) {
    var grid = el('div', 'monopoly-board');
    var properties = propertyBySquare(snapshot);
    for (var square = 0; square < BOARD.length; square++) {
      var position = coords(square);
      var cell = el('div', 'board-square square-' + square);
      cell.style.gridRow = String(position[0]);
      cell.style.gridColumn = String(position[1]);
      cell.title = BOARD[square];
      if (COLORS[square]) cell.appendChild(el('span', 'property-band c-' + COLORS[square]));
      cell.appendChild(el('span', 'square-name', BOARD[square]));
      var property = properties[square];
      if (property && property.owner != null) {
        cell.classList.add('owned', 'owner-' + property.owner);
        if (property.mortgaged) cell.classList.add('mortgaged');
        if (property.houses) {
          cell.appendChild(el('span', 'houses', property.houses === 5 ? '🏨' :
            '⌂'.repeat(property.houses)));
        }
      }
      var tokens = el('span', 'square-tokens');
      (snapshot && snapshot.players || []).forEach(function (player) {
        if (!player.bankrupt && player.position === square) {
          var token = el('span', 'board-token p' + player.player_id, String(player.player_id + 1));
          token.title = (seatById(payload, player.player_id) || {}).label || '';
          tokens.appendChild(token);
        }
      });
      cell.appendChild(tokens);
      grid.appendChild(cell);
    }

    var center = el('div', 'monopoly-board-center');
    center.style.gridRow = '2 / 11';
    center.style.gridColumn = '2 / 11';
    center.appendChild(el('span', 'board-kicker', 'AI MONOPOLY'));
    var phase = PHASE[snapshot.phase] || snapshot.phase || 'Masa';
    center.appendChild(el('strong', 'board-phase', phase));
    var detail = [];
    if (snapshot.last_dice) detail.push('Zar ' + snapshot.last_dice.join(' + '));
    detail.push('Tur ' + (snapshot.round == null ? payload.round || 0 : snapshot.round) +
      '/' + payload.max_rounds);
    center.appendChild(el('span', 'board-detail', detail.join(' · ')));
    grid.appendChild(center);
    return grid;
  }

  function eventLog(payload) {
    var panel = el('section', 'panel monopoly-events');
    panel.appendChild(el('h2', null, 'Hamle akışı'));
    var list = el('ol', 'event-list');
    events.slice(-40).reverse().forEach(function (event) {
      var seat = seatById(payload, event.acted_player) || {label: 'Koltuk ' + (event.acted_player + 1)};
      var row = el('li', 'event-row p' + event.acted_player);
      row.appendChild(el('span', 'event-seq', '#' + event.seq));
      row.appendChild(el('b', null, seat.label));
      row.appendChild(el('span', 'event-action', actionText(event.action_desc)));
      var decision = event.decision_us == null ? 'zorunlu' :
        (Number(event.decision_us) / 1000).toFixed(1) + ' ms';
      row.appendChild(el('span', 'event-round', 'Tur ' + event.round + ' · ' + decision +
        (event.strike ? ' · strike' : '')));
      list.appendChild(row);
    });
    if (!events.length) list.appendChild(el('li', 'muted', 'İlk hamle bekleniyor…'));
    panel.appendChild(list);
    return panel;
  }

  function stopReplay() {
    if (replayTimer) clearTimeout(replayTimer);
    replayTimer = null;
  }

  function showReplayStep(target, keepPlaying) {
    if (!latestPayload || !pinnedGameId) return;
    target = Math.max(1, Math.min(Number(latestPayload.action_count || 1), Number(target)));
    fetch('/ai-monopoly/live?game_id=' + encodeURIComponent(pinnedGameId) + '&at=' + target,
      {headers: {'Accept': 'application/json'}})
      .then(function (response) {
        if (!response.ok) throw new Error('status ' + response.status);
        return response.json();
      })
      .then(function (payload) {
        var event = (payload.events || [])[0];
        if (!event || !event.snapshot) {
          stopReplay();
          return;
        }
        replayAvailable = true;
        replaySeq = event.seq;
        events = [event];
        lastSnapshot = event.snapshot;
        latestPayload = payload;
        render(payload);
        if (keepPlaying && replaySeq < Number(payload.action_count || 0)) {
          replayTimer = setTimeout(function () { showReplayStep(replaySeq + 1, true); }, 250);
        } else {
          stopReplay();
        }
      })
      .catch(stopReplay);
  }

  function replayControls(payload) {
    var controls = el('div', 'monopoly-replay-controls');
    var total = Number(payload.action_count || 0);
    function button(cls, label, title, handler) {
      var node = el('button', 'btn-outline small ' + cls, label);
      node.type = 'button';
      node.title = title;
      node.setAttribute('aria-label', title);
      node.addEventListener('click', handler);
      controls.appendChild(node);
      return node;
    }
    button('replay-first', '⏮', 'İlk hamle', function () {
      stopReplay(); showReplayStep(1, false);
    });
    button('replay-prev', '←', 'Önceki hamle', function () {
      stopReplay(); showReplayStep((replaySeq || total) - 1, false);
    });
    button('replay-play', '▶', 'Tekrarı oynat', function () {
      if (replayTimer) {
        stopReplay();
        return;
      }
      showReplayStep(Math.min(total, (replaySeq || 0) + 1), true);
    });
    button('replay-next', '→', 'Sonraki hamle', function () {
      stopReplay(); showReplayStep((replaySeq || 0) + 1, false);
    });
    button('replay-last', '⏭', 'Son hamle', function () {
      stopReplay(); showReplayStep(total, false);
    });
    var range = el('input', 'replay-range');
    range.type = 'range';
    range.min = '1';
    range.max = String(total);
    range.value = String(replaySeq || total);
    range.setAttribute('aria-label', 'Tekrar hamlesi');
    range.addEventListener('change', function () {
      stopReplay(); showReplayStep(range.value, false);
    });
    controls.appendChild(range);
    controls.appendChild(el('output', 'replay-position',
      'Hamle ' + (replaySeq || total) + ' / ' + total));
    return controls;
  }

  function render(payload) {
    var snapshot = lastSnapshot;
    arena.textContent = '';
    arena.setAttribute('aria-live', 'polite');

    var status = STATUS[payload.status] || [payload.status || 'Bekliyor', 'st-pending'];
    var head = el('div', 'monopoly-live-head');
    var title = el('div');
    title.appendChild(el('p', 'eyebrow', 'DÖRT KOLTUK · TEK TAHTA'));
    var shownRound = snapshot && snapshot.round != null ? snapshot.round : payload.round || 0;
    title.appendChild(el('h2', null, 'Maç ' + payload.game_no + ' · Tur ' +
      shownRound + '/' + payload.max_rounds));
    head.appendChild(title);
    head.appendChild(el('span', 'substatus ' + status[1], status[0]));
    arena.appendChild(head);
    arena.appendChild(playerCards(payload, snapshot));

    if (snapshot) {
      var layout = el('div', 'monopoly-arena-layout');
      var boardScroll = el('div', 'monopoly-board-scroll');
      boardScroll.tabIndex = 0;
      boardScroll.setAttribute('role', 'region');
      boardScroll.setAttribute('aria-label', 'Monopoly tahtası; yatay kaydırılabilir');
      boardScroll.appendChild(board(payload, snapshot));
      layout.appendChild(boardScroll);
      layout.appendChild(eventLog(payload));
      arena.appendChild(layout);
    } else {
      arena.appendChild(el('div', 'arena-idle', 'Worker ilk hamleyi hazırlıyor…'));
    }

    if (replayMode && replayAvailable && Number(payload.action_count || 0) > 0) {
      arena.appendChild(replayControls(payload));
    }

    var atFinalStep = !replayMode || replaySeq == null || replaySeq >= Number(payload.action_count);
    if (payload.status === 'done' && atFinalStep) {
      var winner = seatById(payload, payload.winner_seat);
      var duration = payload.duration_us == null ? '' :
        ' · ' + (Number(payload.duration_us) / 1000000).toFixed(2) + ' sn';
      arena.appendChild(el('div', 'monopoly-result',
        (winner ? '🏆 ' + winner.label + ' kazandı' : 'Uygun kazanan yok') + duration));
    } else if (payload.status === 'failed') {
      arena.appendChild(el('div', 'monopoly-result failed', payload.error_log || 'Oyun tamamlanamadı.'));
    }
  }

  function apply(payload, wasInitial) {
    if (!payload || !payload.id) return;
    if (gameId !== payload.id || attempt !== payload.attempt) {
      gameId = payload.id;
      attempt = payload.attempt;
      seq = 0;
      events = [];
      lastSnapshot = null;
    }
    if (!lastSnapshot && payload.final_snapshot) lastSnapshot = payload.final_snapshot;
    (payload.events || []).forEach(function (event) {
      if (event.seq <= seq) return;
      seq = event.seq;
      events.push(event);
      if (event.snapshot) lastSnapshot = event.snapshot;
    });
    if ((payload.events || []).length) replayAvailable = true;
    if (replayMode && replaySeq == null) replaySeq = seq || Number(payload.action_count || 0);
    latestPayload = payload;
    if (events.length > 200) events = events.slice(-200);
    render(payload);
    if (payload.status === 'done' || payload.status === 'failed' || payload.status === 'cancelled') {
      clearInterval(timer);
      timer = null;
    } else if (!wasInitial && (payload.events || []).length === 100) {
      setTimeout(refresh, 0);
    }
  }

  function refresh() {
    var wasInitial = initialRequest;
    initialRequest = false;
    var query = wasInitial ? '?tail=true' : '?seq=' + seq;
    query += pinnedGameId ? '&game_id=' + encodeURIComponent(pinnedGameId) : '';
    fetch('/ai-monopoly/live' + query, {headers: {'Accept': 'application/json'}})
      .then(function (response) {
        if (!response.ok) throw new Error('status ' + response.status);
        return response.json();
      })
      .then(function (payload) { apply(payload, wasInitial); })
      .catch(function () {});
  }

  // Fetch once even for a terminal game so the server-rendered skeleton gets its final board.
  refresh();
  if (arena.dataset.poll === 'true') timer = setInterval(refresh, 2000);
})();
