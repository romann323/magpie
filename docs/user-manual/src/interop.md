# Working with other tools

Your tags and titles aren't locked inside Magpie. For file formats
Magpie can write into (JPEG, PNG, WebP, GIF), Magpie saves them the
standard way — inside the file itself — so other tools can read
them too, and Magpie can read tags added by other tools.

## Windows Explorer

- **Tags you add in Magpie show up in Explorer.** Right-click a
  file → **Properties** → **Details** tab. Your tags are in the
  *Tags* row and your title in *Title*.
- **Tags you add in Explorer show up in Magpie.** After you type a
  tag in Explorer's Details tab, click **Rescan** in Magpie (or
  wait for the next time you click that file). The tag appears.

> **Note.** Explorer caches things aggressively. If a tag you just
> added isn't showing, press F5 in the folder or navigate away and
> back.

## Windows Photos app

The built-in Photos app reads titles that Magpie wrote. Not all
metadata is editable there, but you'll see it.

## Adobe Lightroom, Adobe Bridge, digiKam

These are photo tools used by pros and enthusiasts. All of them
read the same XMP tag/title format Magpie uses. So a library you
tag in Magpie opens up in Lightroom with everything already
labelled — and vice versa. Any star ratings or captions those tools
wrote are **preserved** by Magpie when it saves your edits, even
though the current Magpie UI doesn't show them.

Some tools cache their view of a folder. If your changes aren't
appearing, use the "refresh" or "reload metadata" option in the
other tool.

## Other tools

Many smaller viewers (XnView, FastStone, IrfanView, and so on) can
read tags in this standard format too. If your tool has a "read XMP
metadata" option, turn it on.

## What if I tag a RAW file, HEIC, PDF, or video?

Some file formats don't have a safe standard way for Magpie to embed
tags inside them yet, so **for these files Magpie stores your tags
in its own library only**. The tag will still appear in Magpie —
the file itself is left untouched.

See [Supported file formats](./file-formats.md) for the full
read-only list. If you shoot RAW+JPEG, Magpie can tag the JPEG
version instead, and those tags are then visible in Windows
Explorer, Lightroom, and every other standard tool.

## Migrating from an older Magpie or Lightroom

Earlier Magpie versions (and Adobe Lightroom for RAW files) sometimes
left small `.xmp` companion files next to your files. Magpie still
reads those on first scan, so you keep your existing tags — but the
first time you save an edit, Magpie embeds the tags into the file
itself and removes the leftover `.xmp`. From that point on, the file
is the only place your tags live.

## In short

If your other tool speaks the standard XMP metadata language (and
almost every modern one does), it reads and writes the same tags and
titles Magpie does — as long as the file format allows embedded XMP.
You can move between tools freely without losing your work.
