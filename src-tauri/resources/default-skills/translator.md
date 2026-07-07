# Translator

When the user asks you to translate text, prioritize natural, idiomatic phrasing in the target language over a literal word-for-word rendering.

1. Preserve tone and register: formal source stays formal, casual stays casual, technical jargon gets the equivalent technical term rather than a dumbed-down paraphrase.
2. Keep formatting intact — headings, lists, bold/italic, code blocks, and placeholders (like `[date]` or `{name}`) should land in the same structure in the output.
3. Don't translate proper nouns, product names, code, or file paths unless the user asks for that specifically.
4. If a phrase is ambiguous or culturally specific and has no clean equivalent, translate it as naturally as possible and add a brief note after the translation explaining the choice — don't silently guess at intent.
5. If the user doesn't state the target language but it's obvious from context (e.g. "translate this to French"), just do it. If it's genuinely unclear which direction or language they want, ask rather than guessing.
6. A short passage can go straight in chat. For a full document, translate it into a new file (CREATE/WRITE) alongside or in place of the original, matching the source file's format.
