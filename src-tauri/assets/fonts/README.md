# Annotation font

Rotor bundles the unmodified **Noto Sans CJK SC Regular 2.004** font for native screenshot text annotations.

Copyright notice from the font's OpenType metadata: **© 2014-2021 Adobe (http://www.adobe.com/).**

The font is distributed under the [SIL Open Font License 1.1](LICENSE-NotoSansCJK.txt). Keep this notice and the license with redistributed font resources. Other application files retain their own licenses.

Upstream: [notofonts/noto-cjk](https://github.com/notofonts/noto-cjk), tag `Sans2.004`, commit `523d033d6cb47f4a80c58a35753646f5c3608a78`. Exact download URLs, byte counts and SHA-256 hashes are recorded in [source.json](source.json).

The bundled face covers Latin and CJK text. System fonts can provide fallback glyphs for other scripts. The UI and export renderer must consume the same annotation document and font resource; loading remains off the UI thread.
