# Where your data lives

A quick, plain-language tour of what Magpie puts where. You don't
need to know any of this to use the app — but many people like
knowing exactly where their information ends up.

## Your files

**Your files stay where you put them.** Magpie never moves, renames,
or changes them (except to write tags into ones that support it — see
below). If you added `C:\Users\me\Pictures\Vacation 2024`, that's
where the files still live.

## Your tags and titles

Whenever you tag a file or give it a title, Magpie tries to save
that **inside the file itself**, using the standard XMP metadata
format that Windows Explorer, Adobe Lightroom, and other tools also
understand. A tag you add in Magpie immediately shows up in File
Explorer's *Details* tab.

Right now Magpie can embed tags into:

- **JPEG** (`.jpg`, `.jpeg`)
- **PNG** (`.png`)
- **WebP** (`.webp`)
- **GIF** (`.gif`, GIF89a only)

**Your files still open and look exactly the same** — only a small
note about the file is added.

**No sidecar files.** Magpie does **not** create any extra `.xmp`
files next to your files. Everything lives inside the file itself.

### File types where tags stay in Magpie only

For everything else (RAW, HEIC, TIFF, PDF, video, …) Magpie **still
lets you tag the file** — the tag just gets remembered in Magpie's
own library instead of embedded in the file. See
[Supported file formats](./file-formats.md) for the full list.

The trade-off: those tags are visible everywhere in Magpie, but if
you copy the file to another computer they don't come along.

### Legacy `.xmp` files

If you used an older version of Magpie or Adobe Lightroom, some of
your files may already have a small `.xmp` companion file next to
them. Magpie still **reads** those on first scan so you don't lose
your existing tags. As soon as you save an edit to that file,
Magpie embeds the new tags into the file itself and **deletes the
leftover `.xmp` file** — from then on the file is the single
source of truth.

## Magpie's own workshop

Magpie keeps its own private files in a hidden Windows folder:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\
```

That folder has:

- A little database that remembers what's in your library, so opening
  the app is instant.
- Tags for read-only formats (RAW, HEIC, PDF, video, …) that can't
  be embedded in the source file.
- Small thumbnail images so the grid loads without delay.
- A log file that helps if you ever run into trouble.

**None of your original files live in this folder.** You can delete
the whole folder if you want — Magpie will just re-scan your library
next time. Tags on read-only formats will be lost.

## The three golden rules

1. **Magpie never moves your files.** They stay in the folders where
   you put them.
2. **Magpie never uploads anything.** No internet, no cloud, no
   account.
3. **For writable formats, tags live inside the file.** So if you
   copy a JPEG or PNG to another computer, its tags come along for
   the ride — no extra files needed.

## Backing up

To back up your library:

1. **Back up your file folders.** Tags for JPEG/PNG/WebP/GIF are
   inside the files themselves — that covers most photos.
2. **Also back up Magpie's workshop folder** if you tagged read-only
   files (RAW, PDF, video…) — those tags live only there.

So "copy my Pictures folder and `%APPDATA%\com.magpie.app` to a
backup drive" is a complete backup.
