# Editing metadata

**Metadata** is a fancy word for "information about a file" — its
title, tags, and filename. It's what makes a folder of `IMG_1234.jpg`
and `document_final_v3.pdf` files feel like a real library.

To edit any of this, click one file in the grid. The **details
panel** on the right lights up with everything you can change.

## What the panel shows

The details panel is split into sections, top to bottom:

1. **Title** — a short, editable name (e.g. "Anna's birthday cake").
2. **Your tags** ✏️ — little labels **you** add inside Magpie:
   `beach`, `family`, `2024`, `contract`. Fully editable. A tiny
   pencil next to the section header shows it's the editable one.
3. **Automatic tags** 🔒 — labels Magpie found **already stored in
   the file itself** when it scanned the folder (for example, tags
   you set in Windows Explorer, Adobe Lightroom, or Bridge), plus
   anything Magpie's built-in auto-tagger contributed if you turned
   it on. This section is **always shown** — with a small lock icon
   in the header and each pill — so the distinction stays visible on
   every file. It's **read-only** here: no × on the pills, no way to
   type into the box. See
   [Automatic vs. your tags](#automatic-vs-your-tags) below for how
   to change them.
4. **Format metadata** — the name of the file's format handler
   (JPEG, PNG, PDF, MP4, …). This will grow as Magpie learns to edit
   more format-specific properties.
5. **File info** — everything Magpie learned from the file itself:
   size, resolution, when it was taken, camera used, page count, video
   duration, and so on. Most of it is read-only; **the filename row
   is editable** — see below.

## Everything saves as you type

There is **no Save button**. Magpie saves your changes automatically:

- **Title**: saved a moment after you stop typing (or as soon as you
  click somewhere else).
- **Tags**: saved immediately once you press space, Enter or comma.
- **Filename**: saved when you press **Enter**; **Esc** or clicking
  away reverts to the previous value.

You'll see a tiny "Saving…" note when it's happening. It usually
takes less than half a second.

## Setting a title

Click the **Title** box, type a name, click somewhere else (or wait a
moment). Done.

To clear a title, just erase what's there.

## Renaming the file

Under **File info**, the **Filename** row is a text field. Edit it
however you like and press **Enter** — Magpie renames the file on
disk **and** updates its record in the project in one step. If the
new name would collide with another file in the same folder, or
contains characters Windows won't allow (`\ / : * ? " < > |`), the
rename is refused and the field turns back to the old name.

**Esc** or clicking away without pressing Enter throws your edit
away.

Just changed a filename you shouldn't have? Use **Edit → Undo**
(Ctrl + Z) — Magpie will rename the file back for you.

## Adding tags

Click the **Your tags** box. Type a word — for example `family` —
and then press one of:

- **Space**
- **Enter**
- **Comma**

The tag becomes a little pill you can see. Type another word, press
space, and there's another. Keep going for as many tags as you want.

While you type, Magpie suggests tags you've already used elsewhere,
so you don't end up with `Beach`, `beach`, and `beaches` all meaning
the same thing. Use the arrow keys to pick a suggestion and press
Enter.

To **remove a tag**, hover over its pill and click the little **×**.
This only removes it from **Your tags**. If the same word also shows
up under **Automatic tags**, it stays there and the file still shows
up in searches for that tag.

## Automatic vs. your tags

Magpie splits tags into two kinds and shows both, in their own
section, on every file:

- **Your tags** ✏️ — anything you typed in Magpie's details panel.
  Editable at any time.
- **Automatic tags** 🔒 — anything the file **already had** when
  Magpie first scanned it: keywords added in Windows Explorer's
  Properties → Details, tags from Adobe Lightroom or Bridge, or
  entries in a Lightroom `.xmp` sidecar next to the file. This
  section is **disabled for editing**: you can read the tags, hover
  them, and search on them, but there's no × on the pills and no
  input box. The container has a dashed outline and each pill
  carries a small lock icon so it's obvious at a glance that it's
  read-only. If the file has no such tags the section still shows,
  with a "No automatic tags on this file." note, so the distinction
  is never hidden.
  **To change or remove an automatic tag, edit the file in the
  original tool and rescan the folder** — Magpie will then pick up
  whatever the file says.

Both kinds count together when you search or when you look at the
sidebar bubbles: a file tagged `beach` shows up whether the tag
came from you or from the file itself, and Magpie doesn't
double-count if the same word is present in both categories.

Rescanning a folder never wipes your typed tags. If a file's own
metadata changes between scans, Magpie will pick up any new
automatic tags but leaves your entries alone.

## Every file is taggable

You can tag anything Magpie recognises — JPEG, PNG, RAW, HEIC, MP4,
PDF, everything. There is no "this file type can't be tagged"
warning, because tags never touch the source file: they live in your
project file (see [Where your data lives](./data-storage.md)).

## Undo / Redo

**Edit → Undo** (Ctrl + Z) reverses your most recent title change,
tag change, or rename. **Edit → Redo** (Ctrl + Y) reapplies it. The
history is per session — it clears when you close the project or
quit the app.

## If a save fails

If a save fails, a red note appears under the field explaining what
went wrong. The usual culprits:

- **Disk full** — free up space.
- **File is locked** by another program (a rename can fail because
  Photoshop or a preview window has the file open). Close the other
  app and try again.
- **Project file can't be written** — very rare; usually a
  permissions issue on the drive that holds your `.magpie` file.
