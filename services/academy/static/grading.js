// Autosave for Görev Puanlama. Every control on this page writes by itself — there is
// no Kaydet button when JS is on (the server renders them inside <noscript>, so with JS
// on they aren't in the DOM at all, and with JS off they are the whole no-JS story).
//
// Deliberately NOT built on admin.js. That script POSTs a form and swaps the entire
// #admin-root innerHTML, which is fine for one-shot panel forms and completely wrong
// here: a swap mid-typing destroys focus, caret position and every other row you were
// part-way through. Nothing on this page ever replaces HTML.
(function () {
  const root = document.getElementById('grading-root');
  if (!root) return;

  const DEBOUNCE = { text: 700, number: 500 };

  // One entry per row currently mid-save, keyed by the row element. `pending` holds the
  // newest form body while a request is in flight, so a fast typist's last keystroke is
  // always what lands — two writes to the same row must never race.
  const inflight = new Map();
  const timers = new Map();

  function bodyFor(form) {
    // The three graded fields live in three different <td>s and are bound to the row
    // form by their `form=` attribute, so FormData(form) already collects them.
    return new URLSearchParams(new FormData(form));
  }

  function setState(form, state, message) {
    const el = form.querySelector('.rowsave');
    if (!el) return;
    el.className = 'rowsave ' + state;
    el.textContent = message || '';
  }

  async function send(form) {
    const body = bodyFor(form);
    if (inflight.has(form)) {
      // already saving — queue the newest state and let the running request chain it
      inflight.get(form).pending = body;
      return;
    }
    const entry = { pending: null };
    inflight.set(form, entry);
    setState(form, 'saving', 'Kaydediliyor…');

    let current = body;
    try {
      for (;;) {
        const res = await fetch(form.action, {
          method: 'POST',
          headers: { 'X-Autosave': '1' },
          body: current,
        });
        if (!res.ok) {
          const msg = (await res.text().catch(() => '')).trim();
          throw new Error(msg || 'Kaydedilemedi.');
        }
        if (!entry.pending) break;
        current = entry.pending;
        entry.pending = null;
      }
      inflight.delete(form);
      flashSaved(form);
      reflectStatus(form);
    } catch (err) {
      inflight.delete(form);
      // Failures do not fade and are clickable — a silently dropped grade is worse
      // than a stuck one.
      setState(form, 'error', err.message || 'Kaydedilmedi — tekrar dene');
      const el = form.querySelector('.rowsave');
      if (el) el.title = 'Tekrar denemek için tıkla';
    }
  }

  function flashSaved(form) {
    setState(form, 'ok', 'Kaydedildi ✓');
    clearTimeout(timers.get(form));
    timers.set(
      form,
      setTimeout(() => {
        if (!inflight.has(form)) setState(form, '', '');
      }, 2000),
    );
  }

  // Marking a row Geçti while the Bekleyenler filter is active must not make it vanish
  // from under the cursor. Recolor its status rail, dim it, and leave it there until the
  // next navigation — the same thing an email client does when you archive a message.
  function reflectStatus(form) {
    const tr = form.closest('tr');
    if (!tr) return;
    const select = tr.querySelector('select[name="status"]');
    if (!select) return;
    tr.dataset.status = select.value;
    const durum = root.dataset.durum || 'bekleyen';
    const matches =
      durum === 'hepsi'
        ? true
        : durum === 'bekleyen'
          ? select.value === 'pending' || select.value === 'reviewing'
          : select.value === durum;
    tr.classList.toggle('row-left-queue', !matches);
  }

  function formFor(el) {
    // Fields bound via form= report their owner in .form; the Site input is inside its
    // own <form>, so this covers both without special-casing.
    return el.form || el.closest('form');
  }

  function schedule(el, delay) {
    const form = formFor(el);
    if (!form || !root.contains(form)) return;
    clearTimeout(timers.get(el));
    timers.set(
      el,
      setTimeout(() => {
        // drop the entry as it fires, or the unload beacon below would re-send a row
        // that already saved
        timers.delete(el);
        send(form);
      }, delay),
    );
  }

  function flush(el) {
    const form = formFor(el);
    if (!form || !root.contains(form)) return;
    clearTimeout(timers.get(el));
    timers.delete(el);
    send(form);
  }

  root.addEventListener('input', (e) => {
    const el = e.target;
    if (el.matches('input[name="points"]')) schedule(el, DEBOUNCE.number);
    else if (el.matches('input[name="feedback"], input[name="live_url"]')) schedule(el, DEBOUNCE.text);
  });

  root.addEventListener('change', (e) => {
    const el = e.target;
    // A status pick is a decision, not a draft — save it the moment it's made.
    if (el.matches('select[name="status"]')) flush(el);
    else if (el.matches('input[name="points"], input[name="feedback"], input[name="live_url"]')) flush(el);
  });

  root.addEventListener(
    'blur',
    (e) => {
      const el = e.target;
      if (el.matches('input[name="points"], input[name="feedback"], input[name="live_url"]')) {
        if (timers.has(el)) flush(el);
      }
    },
    true, // blur doesn't bubble
  );

  // Enter in a text field would otherwise submit the row form and navigate away.
  root.addEventListener('submit', (e) => {
    e.preventDefault();
    send(e.target);
  });

  // Retry a failed row by clicking its indicator.
  root.addEventListener('click', (e) => {
    const el = e.target.closest('.rowsave.error');
    if (el) send(el.closest('form'));
  });

  // The filter chips are ordinary links, so typing a note and immediately clicking one
  // would drop an un-fired debounce. Flush every dirty row through sendBeacon, which is
  // the only request kind the browser guarantees to deliver during unload. The Blob type
  // has to be form-encoded or axum's Form extractor rejects it.
  window.addEventListener('beforeunload', (e) => {
    let failed = false;
    for (const [el, id] of timers) {
      if (!(el instanceof HTMLElement) || !el.matches('input')) continue;
      clearTimeout(id);
      const form = formFor(el);
      if (!form) continue;
      const blob = new Blob([bodyFor(form).toString()], {
        type: 'application/x-www-form-urlencoded',
      });
      if (!navigator.sendBeacon(form.action, blob)) failed = true;
    }
    // Only nag when something actually could not be saved.
    for (const form of inflight.keys()) {
      if (form.querySelector('.rowsave.error')) failed = true;
    }
    if (failed) {
      e.preventDefault();
      e.returnValue = '';
    }
  });

  // Copy one submission's review prompt. Same behaviour as the /admin button it was
  // lifted from; duplicated rather than shared because it is twelve lines and this page
  // has none of admin.js's swap machinery to hang it off.
  root.addEventListener('click', async (e) => {
    const btn = e.target.closest('.btn-copy');
    if (!btn) return;
    try {
      await navigator.clipboard.writeText(btn.dataset.prompt);
    } catch {
      alert('Kopyalanamadı.');
      return;
    }
    const original = btn.textContent;
    btn.textContent = 'Kopyalandı ✓';
    btn.classList.add('btn-saved');
    setTimeout(() => {
      btn.textContent = original;
      btn.classList.remove('btn-saved');
    }, 1400);
  });
})();
