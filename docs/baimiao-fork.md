# Baimiao development fork

> **Last reviewed:** 2026-10-12 · **Last updated:** 2026-10-12 · **Change:** major (local fork and reproducible browser CJK font development) · **Target:** browser DOCX preview and PDF export

This fork is based on `storytold/wordcraft` main at
`1f09547f4e8d59163a355676733e031bcd4180cf` and is hosted at
<https://github.com/12kv/wordcraft>. Browser integration into the Baimiao website is a separate
step; this repository develops and verifies the document engine on its own.

## Local build

The pinned Rust 1.95.0 toolchain matches the minimum required by the locked egui/eframe
dependencies. `rust-toolchain.toml` also selects the WebAssembly target, rustfmt and clippy.
Install Trunk for the web build.

Use the same optional font input as upstream's release workflow. Keep font files in a separate
checkout, with their original OFL licences and manifest hashes:

```sh
git clone https://github.com/storytold/craft-fonts.git ../wordcraft-fonts
git -C ../wordcraft-fonts checkout 8dcdacd5153e64560d109541a47d806f26f048c0
export CRAFT_FONTS_DIR="$(cd ../wordcraft-fonts && pwd)"
export CRAFT_FONTS_REQUIRED=1
cargo test --locked -p wordcraft-fonts
cargo xtask ci
cd apps/wordcraft-web
NO_COLOR=true trunk build --release --locked
```

The static browser build is written to `dist/web`. Serve that directory on localhost or HTTPS
and test both the default WebGPU backend and `?webgl` for WebGL2.

The pinned compiler also exposes pre-existing strict Clippy warnings. The fork uses equivalent
match guards and Boolean expressions in the DOCX/ODT/RTF readers and wildcard search, and lets
a test closure's borrow end at its last use, so these warnings do not block the quality gate.

## Chinese font regression

The official v0.5.0 WebAssembly bundle dropped Simplified Chinese fonts during its build and
only registered Japanese craft-fonts in the document database. A browser cannot rescue missing
characters by scanning system fonts as the native application can.

The regression test uses an empty system font path and explicitly disables system fallback.
It checks the actual bundled Chinese face and the simplified characters that were missing from
Chinese headings, headers, paragraphs and table cells. Optional builds without craft-fonts
continue to support the bundled Latin fonts. Builds with `CRAFT_FONTS_REQUIRED=1` must provide
the required CJK font input.

Browser verification must use the rebuilt artifact through normal file selection and PDF
download. Inspect rendered pages as well as extracted text: missing glyphs can retain a Unicode
mapping, so text extraction alone cannot prove that characters are visible.

This focused font fix does not claim Word-compatible pagination, universal DOCX fidelity,
mobile-device acceptance, or production deployment. Maintain the upstream notices and asset
licences. Before distributing a modified application, remove the ArtCraft marks as required by
`docs/brand/LICENSE-brand.txt`.

## Initial local validation results (2026-10-12)

The original synthetic fixtures below had redundant empty `w:rFonts` style nodes. They
remain useful for the missing-glyph regression, but normal-DOCX pagination comparisons
should use the normalized follow-up results below.

`cargo xtask ci` passed all six gates (format, strict Clippy, tests, assets, layers and WASM).
There were 1,492 passing tests, no failures and one upstream-ignored interface translation
audit. Four build-script contract checks also passed, covering the web font allowlist,
missing Chinese input, absent required checkout and optional font-free builds.

Four synthetic DOCX fixtures were converted with both the rebuilt native CLI and the actual
release browser application through file selection and PDF download. Extracted text retained
the expected Chinese characters, all 80 distinct table row markers and all 240 distinct long
document paragraph markers. PDF font inspection confirms embedded Chinese glyphs. Visual
inspection covered all three Chinese fixture pages, the English page, and the first and last
table/long-document pages. Both WebGPU and WebGL2 displayed Chinese correctly.

| Fixture | LibreOfficeDev 26.8 reference pages | Official v0.5.0 web pages | Fork native pages | Fork web pages |
|---|---:|---:|---:|---:|
| English basics | 1 | 1 | 1 | 1 |
| Chinese layout | 3 | 3 | 3 | 3 |
| Image and 80-row table | 5 | 4 | 5 | 5 |
| 240-paragraph document | 31 | 28 | 28 | 30 |

Pagination remains different from the reference. The browser long-document export places
its last-page header at the top edge with partial clipping, while the native export retains
the margin. These layout issues remain open and are outside this font fix. The reference
is a local LibreOffice development build, not Microsoft Word or the production converter.
Malformed DOCX input was rejected by the rebuilt CLI without an output PDF.

The complete release WASM is 58,080,161 bytes (30,555,257 bytes with gzip level 9). This
includes the GUI and additional upstream main changes, so its size difference from v0.5.0
cannot all be assigned to the font patch. Font loading and bundle size need further work
before website integration. No customer corpus, Safari/Firefox or physical mobile device
acceptance was performed, and no Baimiao website changes or deployment were made.

## Follow-up soffice comparison (2026-10-12)

The fixture generator had appended a second empty `w:rFonts` node in three styles.
Removing those duplicate nodes restores the intended Arial/Songti SC input for both native
engines. All four normalized DOCX files were retested with native WordCraft, actual browser
file selection/PDF download, and headless soffice (LibreOfficeDev 26.8 alpha).

| Normalized fixture | soffice pages | Fork native pages | Fork web pages |
|---|---:|---:|---:|
| English basics | 1 | 1 | 1 |
| Chinese layout | 3 | 3 | 3 |
| Image and 80-row table | 5 | 4 | 5 |
| 240-paragraph document | 31 | 27 | 29 |

All required markers, 80 row numbers and all 480 occurrences of the 240 paragraph numbers
were retained. The original last-page header clipping did not reproduce with normalized
input. It should not be described as a general browser-export defect on normal DOCX files.
Visual review still shows differences in font substitution, paragraph spacing, table rows
and pagination. Neither engine was measured against Microsoft Word in this run.

Three trials with warmed font/file caches gave native WordCraft single-process medians of
48–111 ms versus soffice 1.75–1.86 seconds, including process startup and exit. One soffice
process converting all four files took a median 2.20 seconds total. The native binary is
the current optimized dev build, not a release benchmark. These measurements do not model
the website's persistent unoserver service or browser timings, and memory was not uniformly
lower for WordCraft. Keep the existing formal conversion service while developing the local
browser option and expanding the acceptance corpus.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-12 | minor | Normalize synthetic font styles and compare actual browser/native PDFs with soffice; qualify pagination and startup benchmarks |
| 2026-10-12 | major | Fork baseline, toolchain, font input and browser regression procedure |
