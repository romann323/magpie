# Tags: adding, renaming, cleaning up

Tags are little labels you stick on files so you can find them
later. There are no folders-inside-folders or complicated structures
— tags are just words. A file can have as many as you want.

Good tags are things you'd naturally think of later:

- **Places:** `Iceland`, `beach`, `home`.
- **People:** `Emma`, `Grandad`, `the twins`.
- **Events:** `wedding`, `Christmas-2024`, `hike`.
- **Feelings:** `favourites`, `funny`, `to-print`.

## Two kinds of tags

Magpie shows tags in **two categories** for every file:

- **Your tags** — words you typed yourself in Magpie. Editable
  any time.
- **Automatic tags** — words the file **already had** when Magpie
  first saw it: keywords from Windows Explorer's Properties dialog,
  from Adobe Lightroom, from a `.xmp` sidecar next to the file, and
  so on. When you have
  [Auto-tag photos](./settings.md#auto-tag-photos) turned on,
  Magpie's built-in tagger's suggestions **also land here**, next
  to those, so every read-only, machine-picked tag lives in one
  place. Shown in the details panel as a read-only row (with a lock
  icon) so you can see and search on what the file says — but you
  can't remove them one at a time from Magpie. To change them,
  edit the file in the tool that wrote them and rescan (for XMP /
  Explorer keywords), or turn Auto-tag off and use the sidebar's
  right-click **Delete tag** to sweep an unwanted AI name away
  everywhere.

Everywhere else — sidebar bubbles, searches, the top-of-window search
box — Magpie treats both kinds as one. A file tagged `beach`
appears whether the tag came from you or from the file, and if the
same word is in both categories it isn't counted twice.

See [Editing metadata → Automatic vs. your
tags](./editing-metadata.md#automatic-vs-your-tags) for the full
story.

## Adding tags

The most detailed walkthrough is in
[Editing metadata](./editing-metadata.md#adding-tags). In short:

- Click the **Your tags** box in the details panel.
- Type a word.
- Press **Space**, **Enter**, or **Comma** to save it.

Repeat as many times as you like. To **remove** a tag, click the
little **×** on its pill. Only **Your tags** can be removed like
this; automatic tags don't have an × because they live in the
file itself.

Capital letters don't matter — `Beach` and `beach` are treated as
the same tag.

## The tag cloud in the sidebar

Every tag you've ever used appears as a **bubble** in the **Tags**
section of the sidebar on the left. Bubbles flow across as many rows
as they need. Each bubble carries the tag name and a small number —
how many files carry that tag.

- **Click a bubble** to add that tag to the current search. It turns
  filled in the accent colour so it's easy to spot.
- Click **several** bubbles to narrow with AND logic (files must
  carry *every* selected tag to appear).
- **Click a filled bubble** again to drop it from the search.
- **Clear all** in the Tags header deselects every bubble in one
  click.
- **Right-click a bubble** for two options: **Rename** and
  **Delete**.

Selected tags also appear as removable chips in the search box at
the top of the window — clicking the × on a chip deselects it here.

## Renaming a tag

Made a typo? Change your mind on capitalisation? Right-click the tag
and choose **Rename tag…**. Type the new name and confirm.

Magpie updates the tag on **every file that had it**, all at once.
There's no per-file work to do.

## Deleting a tag

Right-click the tag → **Delete tag**. This removes the tag from every
file — but it does not delete any files. Only the label goes.

## Autocompletion — save yourself typos

While you type a new tag, Magpie shows a small dropdown of tags
you've used before that start with the same letters. Use the arrow
keys to pick one and press Enter. This is the easiest way to keep
your tag list tidy — no more `beach`, `beaches`, `beach`,
`beach-day`.

## Tips for tagging a big library

- **Start broad.** Tag by place or year first — `Iceland`, `2024`.
  Once every file has broad tags, narrow ones (`glacier`, `Reykjavik`)
  are easier because you can filter down first.
- **Use multi-select.** [Ctrl + click](./multi-select.md) many files
  and add tags to them all in one go.
- **Don't over-tag.** A tag that's on 8,000 files isn't very useful.
  When in doubt, pick tags that would show fewer than a few hundred
  results.

## Do tags show up in other apps?

**Only the automatic ones.**

- **Your tags** live inside your **project file** (the `.magpie`
  file you chose the location of). Magpie does not write them into
  the source files, so Windows Explorer, Adobe Lightroom, etc. won't
  see them.
- **Automatic tags** were already stored inside the file by whichever
  tool put them there, so they're still visible in that tool.

The upside is that Magpie **doesn't modify your files** and every
file type is fully taggable inside Magpie. The downside is that if
you want tags to travel with the file (email a JPEG to a friend and
keep its tag visible in their photo viewer), you need to tag it in
Explorer or Bridge instead — Magpie will then pick that tag up as an
**automatic** tag on its next scan. See
[Interoperability with other tools](./interop.md) for the details.
