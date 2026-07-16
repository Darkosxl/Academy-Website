// Watch-time tracker. Heartbeat every 10s while playing; flush on tab hide.
// Page defines: VIDEO_ID (our uuid), YT_ID (youtube id), RESUME_AT (seconds).
let player = null;
let playing = false;
let acc = 0; // seconds actually played since last flush

function onYouTubeIframeAPIReady() {
  player = new YT.Player('player', {
    videoId: YT_ID,
    playerVars: { rel: 0, start: Math.floor(RESUME_AT) },
    events: {
      onStateChange: (e) => {
        playing = e.data === YT.PlayerState.PLAYING;
        if (e.data === YT.PlayerState.PAUSED || e.data === YT.PlayerState.ENDED) flush(false);
      },
    },
  });
}

setInterval(() => { if (playing) acc += 1; }, 1000);
setInterval(() => flush(false), 10000);

function flush(useBeacon) {
  if (!player || !player.getCurrentTime) return;
  if (acc === 0 && !playing) return;
  const body = JSON.stringify({
    video_id: VIDEO_ID,
    position: player.getCurrentTime() || 0,
    duration: player.getDuration() || 0,
    delta: acc,
  });
  acc = 0;
  if (useBeacon && navigator.sendBeacon) {
    navigator.sendBeacon('/api/progress', new Blob([body], { type: 'application/json' }));
  } else {
    fetch('/api/progress', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body });
  }
}

document.addEventListener('visibilitychange', () => { if (document.hidden) flush(true); });
window.addEventListener('pagehide', () => flush(true));
