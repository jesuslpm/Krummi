// Marp CLI configuration for slides.md, loaded automatically when marp-cli runs from this folder:
//   npx @marp-team/marp-cli@4.2.3 slides.md -o slides.html
// It makes every external link (http/https) open in a new tab, so following a "→ Detalle" link during the
// talk never replaces the presentation. In-presentation links (such as #3) are left alone.
// HTML is enabled so the last slide can load presenter-audio.js, the "▶ Nota" button of the presenter view.
export default {
  html: true,
  engine: ({ marp }) =>
    marp.use((md) => {
      const render =
        md.renderer.rules.link_open ||
        ((tokens, idx, options, env, self) => self.renderToken(tokens, idx, options));
      md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
        if (/^https?:\/\//.test(tokens[idx].attrGet('href') || '')) {
          tokens[idx].attrSet('target', '_blank');
          tokens[idx].attrSet('rel', 'noopener noreferrer');
        }
        return render(tokens, idx, options, env, self);
      };
    }),
};
