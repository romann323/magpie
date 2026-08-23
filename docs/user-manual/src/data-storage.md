# Where your data lives

A quick, plain-language tour of what Magpie puts where. You don't
need to know any of this to use the app — but many people like
knowing exactly where their information ends up.

## Your files

**Your files stay where you put them.** Magpie never moves, renames,
or changes them. If you added `C:\Users\me\Pictures\Vacation 2024`,
that's where the files still live, byte-for-byte the way they were.

**Magpie no longer writes into your files.** Older versions of
Magpie tried to embed tags directly into JPEGs and PNGs. That
approach caused problems (files got modified even when the user was
just browsing, some formats couldn't be written to at all, cloud
sync services would re-upload gigabytes for a one-word tag change).
Magpie now leaves your files completely untouched.

## Your tags and titles

Tags and titles now live in a small database file next to your
photos:

```
<your folder>\.magpie\library.db
```

For every folder you add to Magpie, one such `library.db` file is
created inside a hidden `.magpie` subfolder. That file remembers:

- Every image Magpie has seen in this folder.
- The tags and title you've set for each image.
- A search index so typing in the search box is instant.

Because the database sits **inside your folder**, if you copy or
move the whole folder to a different disk (or a different PC), your
tags come along automatically. Magpie will find the `.magpie`
folder the next time you add it to a library.

**Every file becomes taggable.** The old "tags will stay in Magpie
only" warning is gone — RAW files, videos, PDFs, HEIC, TIFF and every
other format are all tagged the same way now, because the tags are
in the database, not in the file.

## Existing tags in your files

If your photos already carry XMP tags (from Lightroom, Bridge, or
older versions of Magpie), or Windows Explorer keywords set through
the *Properties → Details* tab, Magpie **reads them once** on the
first scan and imports them into the folder database. From that
point on the database is the source of truth — later edits in
Magpie stay in the database, and Magpie does **not** try to keep
the file's embedded tags in sync.

If you also edit tags in Explorer or Lightroom, Magpie won't see
those edits until you rescan the folder and manually reimport, so
pick one tool as your master. See
[Interoperability with other tools](./interop.md).

## Folders on OneDrive, Dropbox, or a network share

You can put a Magpie folder on any drive, including cloud-synced
locations like OneDrive, Dropbox, Google Drive, iCloud Drive, or a
network share.

When you add such a folder, Magpie shows a one-time warning: opening
the **same** folder in Magpie from **two PCs at once** can cause
tag edits to be lost, because both PCs are writing into the same
`library.db` file at the same time and the sync client can't merge
them.

Using it from **one PC at a time** is perfectly safe.

## Magpie's own workshop

Magpie also keeps a very small central file in a hidden Windows
folder:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\
```

That folder has:

- A tiny **registry** database that just remembers which folders
  you've added to Magpie (so you don't have to re-add them each
  time the app launches).
- Small **thumbnail images** so the grid loads without delay.
- A **log file** that helps if you ever run into trouble.

**None of your original files, tags, or titles live in this folder.**
You can safely delete the whole folder — Magpie will forget the list
of folders you added, but everything else (photos, tags, titles) is
still safely inside each folder's own `.magpie\library.db`.

## The three golden rules

1. **Magpie never moves or modifies your original files.** They stay
   byte-for-byte the way they were.
2. **Magpie never uploads anything.** No internet, no cloud, no
   account.
3. **Tags and titles travel with the folder.** Copy the folder to
   another disk — its `.magpie\library.db` goes with it, so your
   tags follow.

## Backing up

To back up your library, back up your folders. That's it.

Each folder now carries its own tag database inside it (in the
hidden `.magpie` subfolder), so any backup that grabs the folder
grabs your tags too. The workshop folder at
`%APPDATA%\com.magpie.app` only remembers which folders are
registered — no tag data is lost if it's wiped.
