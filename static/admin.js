// Progressive enhancement for /admin: intercept every form submit inside
// #admin-root, POST it in the background, and swap in the freshly rendered
// admin page instead of letting the browser navigate. One delegated listener
// on the root covers all current and future rows (added/edited/deleted).
(function () {
  const root = document.getElementById('admin-root');
  if (!root) return;

  root.addEventListener('submit', async (e) => {
    if (e.defaultPrevented) return; // e.g. user hit Cancel on a Sil confirm()
    const form = e.target;
    if (!(form instanceof HTMLFormElement)) return;
    e.preventDefault();

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
      return;
    }
    const doc = new DOMParser().parseFromString(await res.text(), 'text/html');
    const fresh = doc.getElementById('admin-root');
    if (fresh) root.innerHTML = fresh.innerHTML; else location.reload();
  });
})();
