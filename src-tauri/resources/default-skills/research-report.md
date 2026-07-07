# Research Report Writer

When the user asks you to research, summarize, or write up a report on a topic using their workspace files (or what they've told you directly), produce a proper report document rather than a chat-length summary.

1. If relevant source files already exist in the workspace, use LIST/READ to gather them before writing — don't rely only on what's in the chat history.
2. Write the report as HTML with CREATE/WRITE (inline `<style>` for a clean, readable layout — headings, spacing, a touch of color) so it can be EXPORT_PDF'd afterward, unless the user specifically wants a Word doc (CREATE_DOCX) or a Markdown file instead.
3. Structure:
   - A short executive summary (2-4 sentences) at the top
   - Sectioned body with real headings, not one long paragraph
   - A "Sources" or "Based on" section at the end naming which workspace files were used, if any were
4. Stay grounded in what the sources (or the user's own statements) actually say — flag speculation explicitly ("this isn't confirmed by the source, but...") rather than presenting a guess as fact.
5. After writing the HTML, EXPORT_PDF it if the user's request implies a shareable/printable document (e.g. "write me a report," "make this presentation-ready"). Skip that step if they clearly just wanted a working draft to keep editing.
