# Settings

Open **Settings** on the menu bar to choose your theme, font size,
and language. Each choice is stored on your PC and applies
immediately — no restart needed.

## Theme

- **Follow system** (default) — matches whichever mode Windows is
  running.
- **Dark** — always the classic dark UI.
- **Light (preview)** — a lighter palette. This is a preview; some
  spots may still look off and we're polishing them.

## Font size

Pick **Small**, **Medium** (the default), or **Large** to scale
labels, filenames, and metadata. If you're on a high-DPI screen and
find text tiny, **Large** is the friend you're looking for.

## Language

Magpie currently ships in English only. This dialog will grow as we
add more languages.

## Auto-tag photos

When this is on, Magpie automatically suggests a couple of tags for
every photo in a folder as soon as you add that folder to a project.
The suggestions land in the photo's **Automatic tags** list — the
same read-only section where tags read from the file itself live —
so they are clearly marked as machine-picked and can't be removed one
at a time from Magpie. To sweep an unwanted AI tag off every photo
at once, right-click it in the sidebar and pick **Delete tag**.

- **Off by default.** Nothing runs unless you turn it on.
- Toggle it from **Settings → Auto-tag photos** on the menu bar.
  A checkmark (✓) at the end of the label means it's on.
- The work happens on your PC — no photo ever leaves your machine.
- You'll see a green **Auto-tagging** progress bar in the status
  bar at the bottom of the window while it runs, next to the usual
  Scanning bar. Magpie stays fully responsive while it works.
- If you add several folders in a row, Magpie tags them one after
  the other so your PC doesn't grind to a halt.
- Adding the same folder again later, or rescanning, doesn't re-do
  photos that haven't changed since Magpie last tagged them.
- Only the initial add of a folder triggers auto-tagging. A plain
  rescan doesn't.

The first version ships with a simple built-in tagger that picks
from a fixed short list of everyday tags (landscape, portrait,
indoor, outdoor, day, night, nature, city, water, food, people,
animal). It's a starting point — a smarter model can slot in later
without changing anything you see.

If you decide auto-tag isn't for you, toggle it off. Tags that were
already added stay put in **Automatic tags** on the affected photos.
Because that section is read-only, you can't clean them up
photo-by-photo from the details panel — instead right-click the
name in the sidebar's tag cloud and pick **Delete tag** to remove
it from every file at once.

## Where these live

Your choices are written to:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\app-settings.json
```

The same file also remembers your most recently opened projects and
which project should re-open on next launch. It never contains
anything private — feel free to delete it to reset to defaults.
