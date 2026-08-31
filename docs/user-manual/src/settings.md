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

When this is on, Magpie looks at every photo in a folder as soon as
you add that folder to a project and picks a few tags that describe
what's in the picture (like *beach*, *dog*, *sunset*, *portrait*).
The tags land in the photo's **Automatic tags** list — the same
read-only section where tags read from the file itself live — so
they're clearly marked as machine-picked. To sweep an unwanted
auto tag off every photo at once, right-click it in the sidebar
and pick **Delete tag**.

### Turning it on

Open **Settings → Auto-tag photos...** on the menu bar. The dialog
walks you through two steps:

1. **Download the AI model.** The first time you use auto-tagging,
   Magpie needs to download the picture-recognition model
   (~580 MB). Click **Download AI model** and wait for the green
   progress bar to fill. This is a one-time download — Magpie
   remembers the model between sessions.
2. **Turn on the toggle.** Once the model is ready, tick
   *Automatically tag photos when adding a new folder*. The toggle
   stays greyed out until step 1 finishes.

### What happens after

- Everything runs on your PC — no photo, no thumbnail, and no tag
  ever leaves your machine. An internet connection is only needed
  once, to download the model.
- You'll see a green **Auto-tagging** progress bar in the status
  bar at the bottom of the window while it runs, next to the usual
  Scanning bar. Magpie stays fully responsive while it works.
- If you add several folders in a row, Magpie tags them one after
  the other so your PC doesn't grind to a halt.
- Adding the same folder again later, or rescanning, doesn't re-do
  photos that haven't changed since Magpie last tagged them.
- Only the initial add of a folder triggers auto-tagging. A plain
  rescan doesn't.
- If you added a folder before you finished downloading the model,
  the status bar shows a small warning ("*AI model not downloaded*").
  Open **Settings → Auto-tag photos...** to finish the download,
  then use **Rescan** on the folder — or just add a new folder —
  to try again.

### About the AI model

Magpie uses **CLIP** — an open-source model from OpenAI that
learned to match photos with plain-English descriptions. It picks
tags from a built-in list of about a thousand common photo words
(scenes, objects, animals, activities, lighting, colours…) and
keeps only the ones it's confident about.

The model isn't perfect: it can miss things, and once in a while
it'll pick a wrong tag. That's fine — the tags are just a
starting point. You can always add your own in the **Tags** box
of the details panel, and use the sidebar's **Delete tag** menu
to clean up any auto tag you don't like across your whole library.

### Removing the model

If you decide you don't want auto-tagging any more, open
**Settings → Auto-tag photos...** and click **Remove model
files**. This frees up ~580 MB on your PC and also turns the
auto-tag toggle off. Existing auto tags on your photos stay in
place — remove them from the sidebar's tag cloud if you want a
clean slate.

## Where these live

Your choices are written to:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\app-settings.json
```

The same file also remembers your most recently opened projects and
which project should re-open on next launch. It never contains
anything private — feel free to delete it to reset to defaults.
