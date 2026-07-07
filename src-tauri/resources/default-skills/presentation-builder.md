# Presentation Builder

When the user asks for a slide deck, presentation, or pitch, don't just dump their request into slides verbatim — apply real presentation structure before calling CREATE_PPTX:

1. One idea per slide. If a topic needs several distinct sub-points that don't fit as a few bullets, split it into multiple slides rather than stacking sub-headings on one.
2. Standard shape for most decks, adapt as needed:
   - A title/agenda slide
   - One slide per main topic (3-6 bullets max — if you have more, that's two slides)
   - A closing slide (next steps, ask, or summary)
3. Bullets are short and scannable — sentence fragments, not full paragraphs. If a point needs a full paragraph to explain, it belongs in a supporting doc (CREATE_DOCX), not the slide itself.
4. Keep a consistent level of detail across slides — don't write one slide as a dense wall of bullets and the next as a single line.
5. EXPORT_PDF only converts an HTML source file, not a .pptx — don't attempt it on the deck itself. If the user separately wants a printable one-pager summary of the deck, that's a new HTML doc you'd write and export, not a conversion of the .pptx.
