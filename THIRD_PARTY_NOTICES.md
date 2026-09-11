# Third-party notices

Rotor is MIT licensed; the project license is distributed as `licenses/Rotor-LICENSE`.

- PP-OCRv6 tiny detector, recognizer and dictionary: PaddleOCR / PaddlePaddle,
  Apache-2.0. See `assets/model/README.md` and `assets/model/LICENSE-APACHE`.
- Google Material Design search icons: Apache-2.0. Original license is retained
  as `licenses/Material-Icons-LICENSE`; source is
  https://github.com/google/material-design-icons.
- GPUI platform adapter: derived from gpui-pre-platform 0.3.3, Apache-2.0.
  See `native/gpui-platform/README.md`; original license is retained as
  `licenses/GPUI-Platform-LICENSE`.
- ONNX Runtime 1.24.2: Microsoft, MIT. Original license and third-party notices
  are distributed under `licenses/runtimes/`. Sources are pinned to
  https://github.com/microsoft/onnxruntime/tree/v1.24.2.
- DirectML 1.15.4: Microsoft. Original runtime license, code license and notices
  from the Microsoft.AI.DirectML 1.15.4 NuGet package are retained in
  `licenses/runtimes/DirectML-*`.
- Rust dependencies: `licenses/dependencies.json` records package version,
  declared license and upstream repository from Cargo's resolved graph.
  `licenses/dependencies/` contains the original license and notice files
  supplied by those packages. This inventory includes build/test dependencies
  and may be broader than the code included in the executable.

System fonts are selected from the host OS and are not bundled with Rotor.
No third-party copyright statements are rewritten.
