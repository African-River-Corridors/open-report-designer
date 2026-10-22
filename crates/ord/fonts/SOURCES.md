# Bundled fonts: sources

Every font here is under the SIL Open Font License 1.1. Each family's licence is next to its files. The
files came from the Google Fonts repository (<https://github.com/google/fonts>), taken on 2026-10-21.

| Family | Upstream project | Version (name table) | Google Fonts path | Licence |
|---|---|---|---|---|
| Public Sans | <https://github.com/uswds/public-sans> | 2.001 | `ofl/publicsans` | `OFL-PublicSans.txt` |
| Libre Caslon Text | <https://github.com/thundernixon/Libre-Caslon> | 2.000 | `ofl/librecaslontext` | `OFL-LibreCaslonText.txt` |
| IBM Plex Mono | <https://github.com/IBM/plex> | 2.3 | `ofl/ibmplexmono` | `OFL-IBMPlexMono.txt` |

## Files

| File | Made from | How |
|---|---|---|
| `PublicSans-Regular.ttf` | `PublicSans[wght].ttf` | static instance, wght=400 |
| `PublicSans-Bold.ttf` | `PublicSans[wght].ttf` | static instance, wght=700 |
| `PublicSans-Italic.ttf` | `PublicSans-Italic[wght].ttf` | static instance, wght=400 |
| `PublicSans-BoldItalic.ttf` | `PublicSans-Italic[wght].ttf` | static instance, wght=700 |
| `LibreCaslonText-Regular.ttf` | `LibreCaslonText[wght].ttf` | static instance, wght=400 |
| `LibreCaslonText-Bold.ttf` | `LibreCaslonText[wght].ttf` | static instance, wght=700 |
| `LibreCaslonText-Italic.ttf` | `LibreCaslonText-Italic[wght].ttf` | static instance, wght=400 |
| `IBMPlexMono-Regular.ttf` | `IBMPlexMono-Regular.ttf` | copied byte for byte, unmodified |
| `IBMPlexMono-Medium.ttf` | `IBMPlexMono-Medium.ttf` | copied byte for byte, unmodified |
| `woff2/*.woff2` | the `.ttf` of the same stem here | woff2 compression, no subsetting |

## Static instances

Public Sans and Libre Caslon Text are published as variable fonts. Word and Typst need static files, so
each weight is a static instance, made with fontTools 4.62.1:

```sh
python3 -m fontTools.varLib.instancer "PublicSans[wght].ttf" wght=700 --update-name-table --static -o PublicSans-Bold.ttf
```

(the same command for each row of the table above, with its source, weight and output). `--update-name-table`
rewrites the name table from the font's STAT table, so each instance has family "Public Sans" or
"Libre Caslon Text" and the style of its weight ("Regular", "Bold", "Italic", "Bold Italic"). The family names
are kept: neither licence declares a Reserved Font Name. No glyph is removed: the TrueType files have the full
glyph set.

IBM Plex Mono's licence declares the Reserved Font Name "Plex", so its files are not modified in any way.

## Woff2 copies

For HTML. Each is the TrueType file of the same stem, compressed and not otherwise changed (fontTools with
brotli):

```sh
python3 -m fontTools.ttLib.woff2 compress -o woff2/PublicSans-Bold.woff2 PublicSans-Bold.ttf
```
