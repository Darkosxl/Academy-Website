// Takım formasyonu: drag a kid card onto a team column. Every action here also exists
// as a plain <form> in the markup, so with JS off the page still works — the drag is
// the shortcut, not the mechanism.
(function () {
  const root = document.getElementById('formation-root');
  if (!root) return;

  // The server's Form extractor only accepts application/x-www-form-urlencoded, so
  // URLSearchParams — a raw FormData would go out as multipart and be rejected.
  async function post(action, fields) {
    let res;
    try {
      res = await fetch(action, { method: 'POST', body: new URLSearchParams(fields) });
    } catch {
      alert('Bağlantı hatası, sayfa yenileniyor.');
      location.reload();
      return;
    }
    if (!res.ok) {
      alert((await res.text().catch(() => '')) || 'Kaydedilemedi.');
      return;
    }
    // ponytail: full reload instead of patching the DOM. The page is a few KB and every
    // move changes two columns' counts; a diff engine here buys nothing.
    location.replace('/admin/takimlar?saved=1');
  }

  // Delegated: a reload replaces every card, but not the root they hang off.
  root.addEventListener('dragstart', (e) => {
    const card = e.target.closest('.kid');
    if (!card) return;
    e.dataTransfer.setData('text/plain', card.dataset.user);
    e.dataTransfer.effectAllowed = 'move';
    card.classList.add('dragging');
  });

  root.addEventListener('dragend', (e) => {
    const card = e.target.closest('.kid');
    if (card) card.classList.remove('dragging');
  });

  // Without preventDefault on dragover the element is not a valid drop target at all.
  root.addEventListener('dragover', (e) => {
    const col = e.target.closest('.fcol');
    if (!col) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    col.classList.add('over');
  });

  root.addEventListener('dragleave', (e) => {
    const col = e.target.closest('.fcol');
    // fires on every child crossing too; only clear when the pointer really left
    if (col && !col.contains(e.relatedTarget)) col.classList.remove('over');
  });

  root.addEventListener('drop', (e) => {
    const col = e.target.closest('.fcol');
    if (!col) return;
    e.preventDefault();
    col.classList.remove('over');
    const userId = e.dataTransfer.getData('text/plain');
    if (!userId) return;
    const teamId = col.dataset.team; // absent on the Takımsız column
    if (teamId) post('/admin/takimlar/member', { user_id: userId, team_id: teamId });
    else post('/admin/takimlar/member/remove', { id: userId });
  });

  // Intercept the create/rename/delete/assign forms so they behave like the drops do.
  // defaultPrevented covers the Sil confirm() being cancelled.
  root.addEventListener('submit', (e) => {
    if (e.defaultPrevented) return;
    const form = e.target;
    if (!(form instanceof HTMLFormElement)) return;
    e.preventDefault();
    post(form.action, new URLSearchParams(new FormData(form, e.submitter)));
  });
})();
