// No-reload Görev Panosu: intercept task-card form submits (Göreve Başla /
// Projenizi Yükle), POST in the background, and swap in the freshly rendered
// board instead of letting the browser navigate. Mirrors static/admin.js; one
// delegated listener covers every card, current and future.
(function () {
  const root = document.getElementById('board-root');
  if (!root) return;

  root.addEventListener('submit', async (e) => {
    if (e.defaultPrevented) return;
    const form = e.target;
    if (!(form instanceof HTMLFormElement)) return;
    e.preventDefault();

    const submitter = e.submitter;
    const originalLabel = submitter ? submitter.textContent : null;
    if (submitter) { submitter.disabled = true; submitter.textContent = 'Yükleniyor…'; }

    // /board/submit takes a file (multipart); /board/interest is urlencoded.
    // Pass the submitter so any name=value on the clicked button is included.
    const multipart = form.enctype === 'multipart/form-data';
    const body = multipart
      ? new FormData(form, submitter)
      : new URLSearchParams(new FormData(form, submitter));

    let res;
    try {
      res = await fetch(form.action, { method: 'POST', body });
    } catch {
      alert('Bağlantı hatası, sayfa yenileniyor.');
      location.reload();
      return;
    }
    if (!res.ok) {
      alert((await res.text().catch(() => '')) || 'İşlem başarısız.');
      if (submitter) { submitter.disabled = false; submitter.textContent = originalLabel; }
      return;
    }
    // The POST handlers 303-redirect to /board; fetch follows it, so the response
    // is the fresh board page — pull #board-root out of it and swap in place.
    const doc = new DOMParser().parseFromString(await res.text(), 'text/html');
    const fresh = doc.getElementById('board-root');
    if (!fresh) { location.reload(); return; }
    root.innerHTML = fresh.innerHTML;
  });
})();
