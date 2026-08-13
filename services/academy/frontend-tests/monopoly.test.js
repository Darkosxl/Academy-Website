'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

class FakeClassList {
  constructor(node) { this.node = node; }
  add(...names) {
    const current = new Set(this.node.className.split(/\s+/).filter(Boolean));
    names.forEach((name) => current.add(name));
    this.node.className = [...current].join(' ');
  }
}

class FakeElement {
  constructor(tag) {
    this.tagName = tag;
    this.children = [];
    this.className = '';
    this.classList = new FakeClassList(this);
    this.dataset = {};
    this.attributes = {};
    this.listeners = {};
    this.style = {};
    this._text = '';
  }
  set textContent(value) {
    this._text = String(value == null ? '' : value);
    this.children = [];
  }
  get textContent() {
    return this._text + this.children.map((child) => child.textContent).join('');
  }
  set innerHTML(_value) { throw new Error('unsafe innerHTML write'); }
  appendChild(child) { this.children.push(child); return child; }
  setAttribute(name, value) { this.attributes[name] = String(value); }
  addEventListener(name, callback) { this.listeners[name] = callback; }
}

const malicious = '</script><img src=x onerror=globalThis.pwned=true>';
const arena = new FakeElement('div');
arena.dataset.gameId = 'game-1';
arena.dataset.poll = 'false';
arena.dataset.replay = 'true';
const document = {
  getElementById(id) { return id === 'monopoly-arena' ? arena : null; },
  createElement(tag) { return new FakeElement(tag); },
};
const snapshot = {
  round: 1,
  phase: 'pre_roll',
  active_player: 0,
  last_dice: [1, 1],
  players: [],
  properties: [],
};
function payload(events) {
  return {
    id: 'game-1',
    attempt: 1,
    game_no: 1,
    status: 'done',
    action_count: 101,
    round: 1,
    max_rounds: 200,
    duration_us: 1_000_000,
    winner_seat: 0,
    final_snapshot: snapshot,
    seats: [
      {player_id: 0, entry_id: 'entry-0', label: malicious},
      {player_id: 1, bot_key: 'hoarder', label: 'Bot 1'},
      {player_id: 2, bot_key: 'builder', label: 'Bot 2'},
      {player_id: 3, bot_key: 'gambler', label: 'Bot 3'},
    ],
    events,
  };
}
const pages = [
  payload([{
    seq: 101,
    acted_player: 0,
    round: 1,
    action_desc: 'END_TURN',
    decision_us: 500,
    strike: false,
    snapshot,
  }]),
  payload([{
    seq: 100,
    acted_player: 0,
    round: 1,
    action_desc: 'ROLL_DICE',
    decision_us: null,
    strike: false,
    snapshot,
  }]),
];
let fetches = 0;
const urls = [];
const context = {
  document,
  fetch(url) {
    urls.push(url);
    const page = pages[fetches++];
    assert.ok(page, 'terminal replay should stop after the short final page');
    return Promise.resolve({ok: true, json: () => Promise.resolve(page)});
  },
  setInterval() { throw new Error('terminal replay must not start interval polling'); },
  clearInterval() {},
  clearTimeout() {},
  setTimeout(callback) { Promise.resolve().then(callback); return 1; },
  encodeURIComponent,
  console,
};

vm.runInNewContext(
  fs.readFileSync(path.join(__dirname, '..', 'static', 'monopoly.js'), 'utf8'),
  context,
  {filename: 'monopoly.js'},
);

(async () => {
  for (let turn = 0; turn < 8; turn += 1) {
    await new Promise((resolve) => setImmediate(resolve));
  }
  assert.equal(fetches, 1, 'terminal replay must load only the latest event tail');
  assert.match(urls[0], /tail=true/, 'initial page must request the tail, not all history');
  assert.ok(arena.textContent.includes(malicious), 'untrusted team names must render as text');
  assert.equal(context.pwned, undefined, 'untrusted team names must never execute');
  function findClass(node, name) {
    if (node.className.split(/\s+/).includes(name)) return node;
    for (const child of node.children) {
      const found = findClass(child, name);
      if (found) return found;
    }
    return null;
  }
  const previous = findClass(arena, 'replay-prev');
  assert.ok(previous, 'terminal replay must render step controls');
  previous.listeners.click();
  for (let turn = 0; turn < 4; turn += 1) {
    await new Promise((resolve) => setImmediate(resolve));
  }
  assert.equal(fetches, 2);
  assert.match(urls[1], /at=100/, 'previous must fetch exactly one earlier step');
  assert.ok(arena.textContent.includes('#100'));
  console.log('monopoly frontend polling/XSS tests ok');
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
