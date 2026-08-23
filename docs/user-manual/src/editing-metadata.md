# Editing metadata

**Metadata** is a fancy word for "information about a file" — its
title and tags. It's what makes a folder of `IMG_1234.jpg` and
`document_final_v3.pdf` files feel like a real library.

To edit any of this, click one file in the grid. The **details
panel** on the right lights up with everything you can change.

## What the panel shows

The details panel is split into four sections, top to bottom:

1. **Title** — a short, editable name (e.g. "Anna's birthday cake").
2. **Tags** — little labels like `beach`, `family`, `2024`,
   `contract`.
3. **Format metadata** — the name of the file's format handler
   (JPEG, PNG, PDF, MP4, …). This will grow as Magpie learns to edit
   more format-specific properties.
4. **File info** — everything Magpie learned from the file itself:
   size, resolution, when it was taken, camera used, page count, video
   duration, and so on. This section is read-only — those numbers
   describe the file, not your notes about it.

## Everything saves as you type

There is **no Save button**. Magpie saves your changes automatically:

- **Title**: saved a moment after you stop typing (or as soon as you
  click somewhere else).
- **Tags**: saved immediately once you press space, Enter or comma.

You'll see a tiny "Saving…" note when it's happening. It usually
takes less than half a second.

## Setting a title

Click the **Title** box, type a name, click somewhere else (or wait a
moment). Done.

To clear a title, just erase what's there.

## Adding tags

Click the **Tags** box. Type a word — for example `family` — and then
press one of:

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

## Every file is taggable

You can tag anything Magpie recognises — JPEG, PNG, RAW, HEIC, MP4,
PDF, everything. There is no "this file type can't be tagged"
warning, because tags never touch the source file: they live in
Magpie's own database (see [Where your data lives](./data-storage.md)).

## If a save fails

If a save fails, the panel shows a red note under the Tags field:

> Save failed: *(the exact error)*

The usual culprits are a full disk or a second Magpie window running
against the same account. Free up space or close the other window
and re-edit the tag.
