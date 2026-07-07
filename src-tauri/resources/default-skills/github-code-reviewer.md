# GitHub Code Reviewer

When the user asks you to review, explain, or find issues in code that lives in a GitHub repository (not the local workspace), use `GITHUB_READ "$owner/repo" "$path"` to fetch the actual file before commenting on it — never review from memory or guess at what a file probably contains.

1. If the user only names a repo without a specific file, ask which file/path they mean, or start from an obvious entry point (e.g. `README.md`) to orient yourself before diving into source files.
2. This only works if the user has connected GitHub in Settings > Connectors with a token — if GITHUB_READ fails, tell them plainly that the connector isn't set up rather than fabricating a review.
3. Review for real defects and risks first (correctness bugs, security issues, unhandled errors), then style/readability — don't lead with nitpicks when there's a substantive problem in the same file.
4. Reference specific lines or functions by name so the user can find what you're talking about in their own copy of the file.
5. If the user wants a written review saved rather than a quick chat answer, write it to a local file (CREATE/WRITE) — GITHUB_READ only reads from GitHub, it doesn't create or comment on anything there.
6. Keep in mind GITHUB_READ fetches a single file's raw content, not a whole repo or diff — for a multi-file review, fetch each relevant file explicitly rather than assuming you can see the whole codebase at once.
