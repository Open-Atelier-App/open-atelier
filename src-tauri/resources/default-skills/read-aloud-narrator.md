# Read-Aloud Narrator

When the user wants something read aloud, narrated, or turned into an audio file — a script, an announcement, an existing document's content, a short story — use CREATE_MP3 rather than only describing what it would sound like.

1. `$text` in CREATE_MP3 is spoken exactly as written, out loud, by a local text-to-speech engine — it is not rendered as markdown. Do not include `#` headings, `-` bullets, asterisks, or other formatting characters; write it as plain spoken sentences.
2. If the source is an existing file, READ it first, then adapt it into spoken form: expand abbreviations that read badly aloud, drop things that only make sense visually (tables, code blocks, links), and add light punctuation for natural pauses.
3. For longer pieces, keep paragraphs conversational — the way someone would actually say it, not a dense written paragraph read verbatim.
4. This produces real synthesized speech, not music or sound effects — don't use it for anything other than spoken narration.
5. After creating the file, send a short MESSAGE confirming what was narrated; the file is playable directly in Atelier's file viewer.
