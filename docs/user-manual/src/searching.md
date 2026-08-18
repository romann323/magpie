# Searching and filtering

## Full-text search

The search box at the top of the window filters the grid live as you
type. PicOrg uses SQLite's FTS5 (full-text search) engine over four
fields:

- `title`
- `comment`
- `filename` (including the extension)
- `tags`

The default matching is **prefix-friendly** and case-insensitive, so
typing `sun` finds photos titled *Sunset*, tagged `sunny`, in a file
called `sunbathing.jpg`, or with a comment mentioning "sunrise".

### Boolean operators

Because FTS5 is under the hood, you can also write more elaborate
queries:

- `vacation beach` — photos matching both terms (AND is implicit).
- `vacation OR beach` — photos matching either.
- `vacation -beach` — vacation photos that are not tagged beach.
- `"family reunion"` — exact phrase.
- `tag:2024` — restrict to a specific column (advanced).

### Diacritics

The tokenizer strips diacritics, so `naive` matches `naïve` and vice
versa.

## Combining search with sidebar filters

Search and sidebar filters compose with AND. For example, showing
"5-star photos from `\Photos\Trips\Iceland` tagged `aurora` mentioning
`glacier`" is:

1. Click the Iceland folder in the sidebar.
2. Click **5 stars and up** in the sidebar.
3. Click the `aurora` tag in the sidebar.
4. Type `glacier` in the search box.

The status bar at the bottom always shows the count of images matching
your current combined filter.

## Clearing the search

Click the `×` inside the search box (or press <kbd>Esc</kbd> while
focused there) to clear the query. Sidebar filters aren't touched.

## Search performance

FTS5 is fast. On a 250 000-photo library, a query typically returns
results in a few milliseconds. If a query ever feels slow, it's
usually because a big filter change is invalidating cached thumbnails
being redrawn — the query itself finishes long before the grid.
