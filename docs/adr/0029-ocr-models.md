# 29. OCR models: optional downloads beside Tesseract

Status: accepted. PaddleOCR-VL built; TeleOCR listed for later.

Tesseract reads scanned pages quickly, in many languages, but as plain lines. Newer document models read a page more like a person does: headings, tables, formulas and charts come out in reading order. They are large and slow, so they are optional.

## Decisions

- **Optional downloads** in Settings › Helper programs › *Reading scanned pages*, each with **Download** and **Delete**. One is chosen at a time; Tesseract stays the default and the fallback.
- **PaddleOCR-VL 1.6** now (PaddlePaddle, Apache 2.0, 0.9 billion parameters, 109 languages).
- **TeleOCR later:** it is listed as *Coming later*. No Rust version of it exists, and a port has to be tested on real scans before Libreri carries it.

## How it works

- New crate `libreri-ocr`: the list of models, downloading (from Hugging Face, file by file, each checked against its length, resumable; a `.complete` note marks a whole download), removing, and loading.
- Models run in Libreri with candle through the `oar-ocr-vl` crate (Apache 2.0): PP-DocLayoutV3 finds the regions of a page and their reading order, PaddleOCR-VL reads each region (text, tables as HTML, formulas as LaTeX, charts). Apple computers use the graphics chip (Metal); others the processor.
- Files live in the app's data folder, `ocr-models/<model>/` (about 2.1 GB for PaddleOCR-VL, with its layout model in `layout/`). They belong to this computer and are never in backups or exports.
- `libreri-formats::ocr::PageReader` is the seam: *Make searchable* reads pages with the chosen model instead of Tesseract, one page at a time, and keeps the engine's name with the text (`.library-data/text/<book>.json`, as before).
- The models tell where each region is, not each word, so words are laid out over their region line by line (`words_in_region`). That is close enough to select, highlight, find and read aloud; Tesseract's word boxes stay exact.
- Choosing another engine frees the loaded model's memory; deleting the chosen one goes back to Tesseract. Text already read is kept either way.
