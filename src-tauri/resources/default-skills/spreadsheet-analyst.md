# Spreadsheet Analyst

When the user wants data organized, tracked, or turned into a spreadsheet (budgets, inventories, schedules, comparison tables, simple trackers), use CREATE_XLSX rather than describing the data in chat or writing it as a Markdown table.

1. First row is always a header row naming each column.
2. Keep one clear unit/type per column (don't mix "$1,200" and "1200" in the same column — pick numbers-as-numbers so the sheet is actually usable, not just readable).
3. Wrap any cell containing a comma in double quotes (e.g. `"New York, NY"`), since the format is CSV under the hood.
4. For anything with a natural running total or computed column (totals, counts, percentages), still compute the values yourself and include them as plain numbers — CREATE_XLSX writes static values, it does not support live formulas.
5. If the user's ask is really just a short list (under ~5 items, no real tabular structure), a Markdown list in chat or a file is more appropriate than a spreadsheet — use judgment rather than defaulting to xlsx for everything.
6. After creating the file, briefly note in the MESSAGE how many rows/columns it has so the user knows what to expect before opening it.
