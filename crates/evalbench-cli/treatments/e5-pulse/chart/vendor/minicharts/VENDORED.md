# Vendored Minicharts

These files are copied unchanged from the public Minicharts skill (MIT, Copyright (c) 2026
April Zhu): `engine.js`, `run.js`, `page.html`, `LICENSE` and `NOTICE.md`. Keep `LICENSE`
and `NOTICE.md` with any copy.

Only the engine is vendored. The 54 example drawings (`examples.js`) and the skill text
are not.

`NOTICE.md` says the fonts load from Google Fonts. That is true of upstream Minicharts, but
not of the E5 page: it bundles Young Serif, Gaegu and DM Sans in `../../fonts/` (SIL OFL)
and loads no font from the network. `scripts/e5/build_chart.py` swaps the Google Fonts links in `page.html` for the
bundled `../fonts/fonts.css` and fails the build if a font-host link is left. The colour
softening in that script is adapted from Minicharts' `build.py`.
