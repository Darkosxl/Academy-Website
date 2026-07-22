// Progressive enhancement for /admin: intercept every form submit inside
// #admin-root, POST it in the background, and swap in the freshly rendered
// admin page instead of letting the browser navigate. One delegated listener
// on the root covers all current and future rows (added/edited/deleted).
(function () {
  const root = document.getElementById('admin-root');
  if (!root) return;

  // Cloudflare's "Email Address Obfuscation" rewrites every raw email address
  // in the HTML response (including this fetch's own response — it's an edge
  // rewrite, not something the server controls) into a data-cfemail-encoded
  // placeholder, then decodes it back client-side via a script it injects that
  // only scans the page once on the real load. Content we splice in afterwards
  // via innerHTML never gets that pass, so we redo the (public, documented)
  // decode ourselves after every swap.
  function cfDecode(hex) {
    const key = parseInt(hex.substr(0, 2), 16);
    let out = '';
    for (let i = 2; i < hex.length; i += 2) out += String.fromCharCode(parseInt(hex.substr(i, 2), 16) ^ key);
    return out;
  }
  function decodeObfuscatedEmails(container) {
    container.querySelectorAll('[data-cfemail]').forEach((el) => {
      const email = cfDecode(el.getAttribute('data-cfemail'));
      el.textContent = email;
      if (el.tagName === 'A') el.href = 'mailto:' + email;
    });
  }

  // The full-page swap on success replaces the button that was clicked, so any
  // "Kaydedildi" flash has to be applied to its replacement. Forms are matched
  // back up by action + the row's hidden id field (absent on add/refresh forms,
  // which are unique per action anyway).
  function flashSaved(action, idValue) {
    const idSel = idValue != null ? `input[name="id"][value="${CSS.escape(idValue)}"]` : null;
    const forms = root.querySelectorAll(`form[action="${CSS.escape(action)}"]`);
    for (const f of forms) {
      if (idSel && !f.querySelector(idSel)) continue;
      const btn = f.querySelector('button');
      if (!btn) continue;
      const original = btn.textContent;
      btn.textContent = 'Kaydedildi ✓';
      btn.classList.add('btn-saved');
      setTimeout(() => { btn.textContent = original; btn.classList.remove('btn-saved'); }, 1400);
      break;
    }
  }

  root.addEventListener('submit', async (e) => {
    if (e.defaultPrevented) return; // e.g. user hit Cancel on a Sil confirm()
    const form = e.target;
    if (!(form instanceof HTMLFormElement)) return;
    e.preventDefault();

    const submitter = e.submitter;
    const originalLabel = submitter ? submitter.textContent : null;
    if (submitter) { submitter.disabled = true; submitter.textContent = 'Kaydediliyor…'; }

    const action = form.getAttribute('action');
    const idField = form.elements.namedItem('id');
    const idValue = idField ? idField.value : null;

    // The server's Form extractor only accepts application/x-www-form-urlencoded,
    // so send URLSearchParams (not a raw FormData, which fetch encodes as
    // multipart/form-data) — fetch then sets the matching Content-Type on its own.
    let res;
    try {
      res = await fetch(form.action, { method: 'POST', body: new URLSearchParams(new FormData(form)) });
    } catch {
      alert('Bağlantı hatası, sayfa yenileniyor.');
      location.reload();
      return;
    }
    if (!res.ok) {
      alert((await res.text().catch(() => '')) || 'Kaydedilemedi.');
      if (submitter) { submitter.disabled = false; submitter.textContent = originalLabel; }
      return;
    }
    const doc = new DOMParser().parseFromString(await res.text(), 'text/html');
    const fresh = doc.getElementById('admin-root');
    if (!fresh) { location.reload(); return; }
    root.innerHTML = fresh.innerHTML;
    decodeObfuscatedEmails(root);
    flashSaved(action, idValue); // no-op if the row was just deleted — its own absence is the feedback
  });
})();
