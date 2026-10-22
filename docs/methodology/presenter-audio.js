// Recorded speaker notes in Marp's presenter view (press P during the talk).
//
// Adds a "▶ Nota" button to the presenter view's bottom bar that plays the current slide's recorded note,
// audio/slide-NN.mp3 (made by scripts/slides/notes_audio.py). A second click pauses it, and changing slide
// stops it. The audience view and the next-slide preview, which is an iframe of this same page, get nothing,
// because only the presenter view has the bottom bar. To send the sound to a Teams meeting, share with
// "Include sound" in the Teams desktop app: it shares all the computer's audio, this window's included.
(() => {
  if (window.top !== window) return;

  const install = (bar, page) => {
    const audio = new Audio();
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'nota-audio';
    button.title = 'Reproducir la nota grabada de esta slide';
    Object.assign(button.style, {
      width: 'auto', padding: '0 0.6em', marginRight: '1em', border: '1px solid #eee', borderRadius: '4px',
      background: 'transparent', color: 'inherit', cursor: 'pointer', font: 'inherit',
    });

    // The bar shows "N / total"; N is the slide on screen.
    const file = () => `audio/slide-${String(parseInt(page.textContent, 10)).padStart(2, '0')}.mp3`;
    const show = () => {
      button.textContent = audio.paused ? '▶ Nota' : '⏸ Nota';
      button.dataset.audio = file(); // not read here: shows in the DOM which file the button would play
    };

    button.addEventListener('click', () => {
      if (!audio.paused) {
        audio.pause();
        return;
      }
      const wanted = new URL(file(), location.href).href;
      if (audio.src !== wanted) audio.src = wanted;
      audio.play().catch((error) => {
        // A pause or slide change before playback starts aborts play(): that is not a missing file.
        if (error.name !== 'AbortError') button.textContent = '✕ Sin audio';
      });
    });
    for (const event of ['play', 'pause', 'ended']) audio.addEventListener(event, show);

    new MutationObserver(() => {
      audio.pause();
      audio.removeAttribute('src');
      audio.load();
      show();
    }).observe(page, { childList: true, characterData: true, subtree: true });

    bar.prepend(button);
    show();
  };

  // Marp builds the presenter view after the page loads: wait for its bar, for a few seconds at most.
  let tries = 50;
  const wait = () => {
    const bar = document.querySelector('.bespoke-marp-presenter-info-container');
    const page = document.querySelector('.bespoke-marp-presenter-info-page-text');
    if (bar && page && page.textContent) install(bar, page);
    else if (--tries > 0) setTimeout(wait, 100);
  };
  wait();
})();
