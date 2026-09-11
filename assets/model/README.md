# Bundled OCR models

These PP-OCRv6 tiny ONNX models and dictionary were introduced into Rotor by
commit `b469c56007a6c95927ed848c4ca0038274e33585` and retained byte-for-byte in 3.0.0.

Upstream model authors: PaddleOCR / PaddlePaddle. The official
[detector](https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_det_onnx) and
[recognizer](https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_rec_onnx) cards
identify Apache-2.0. `LICENSE-APACHE` preserves the upstream PaddleOCR license
from https://github.com/PaddlePaddle/PaddleOCR/blob/main/LICENSE (retrieved 2026-09-11).

The [oar-ocr model catalog](https://github.com/GreatV/oar-ocr/blob/main/docs/models.md)
lists these flat filenames and their upstream PaddleOCR inference bundles;
[oar-ocr v0.7.0](https://github.com/GreatV/oar-ocr/releases/tag/v0.7.0) distributes
this detector, recognizer and 6,904-character tiny dictionary. Rotor uses oar-ocr
for inference. Original download receipts were not retained in the introducing
commit; the hashes below identify the actual bundled files, not a newly fetched
copy of a mutable upstream asset.

| File | Bytes | SHA-256 |
|---|---:|---|
| pp-ocrv6_tiny_det.onnx | 1780590 | 193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8 |
| pp-ocrv6_tiny_rec.onnx | 4462639 | 9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6 |
| ppocrv6_tiny_dict.txt | 27156 | c5cbe34ef40c29c4df07ed012bf96569cb69a2d2a01a07027e9f13cb832bd9cd |

Keep the matching dictionary and model bytes together. No font files are bundled;
text annotations use installed system fonts.
