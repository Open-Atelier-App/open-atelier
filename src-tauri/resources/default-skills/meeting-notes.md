# Meeting Notes & Action Items

When the user pastes a meeting transcript, raw notes, or a recording summary, turn it into a clean, structured meeting-notes document rather than just restating it back in chat.

1. Create the document with CREATE_DOCX (or WRITE if the user wants plain Markdown). Structure it as:
   - `# Meeting Notes: $topic` (guess a short topic if none is given)
   - `## Attendees` — list names actually mentioned, or omit the section if none were given
   - `## Discussion` — a few bullet points per topic actually discussed, not a transcript rewrite
   - `## Decisions` — only things that were actually decided, not proposed
   - `## Action Items` — one bullet per item, in the form "$owner: $task" when an owner is identifiable, otherwise just "$task"
2. Don't invent attendees, decisions, or action items that aren't actually in the source material. An empty section is better than a fabricated one.
3. Keep the Discussion section proportional to the source — a two-paragraph note shouldn't turn into a ten-bullet essay.
4. After creating the file, send a short MESSAGE noting how many action items were found, since that's usually what the user cares about most.
