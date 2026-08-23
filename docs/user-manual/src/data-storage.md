# Where your data lives

A quick, plain-language tour of what Magpie puts where. You don't
need to know any of this to use the app — but many people like
knowing exactly where their information ends up.

## Your files

**Your files stay where you put them.** Magpie never moves, renames,
or changes them. If you added `C:\Users\me\Pictures\Vacation 2024`,
that's where the files still live, byte-for-byte the way they were.

**Magpie never writes into your files.** Older versions of Magpie
tried to embed tags directly into JPEGs and PNGs. That approach
caused problems (files got modified even when the user was just
browsing, some formats couldn't be written to at all, cloud sync
services would re-upload gigabytes for a one-word tag change).
Magpie now leaves your files completely untouched.

## Your tags and titles

Tags and titles live in one small database file that Magpie keeps in
a hidden Windows folder:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\magpie.db
```

That single file remembers:

- Every folder you've added to Magpie.
- Every image Magpie has seen in each folder.
- The tags and title you've set for each image.
- A search index so typing in the search box is instant.
- Any smart searches (saved searches) you've created.

**Every file becomes taggable.** RAW files, videos, PDFs, HEIC,
TIFF and every other format are all tagged the same way now,
because the tags are in Magpie's database, not in the file.

## Existing tags in your files

If your photos already carry XMP tags (from Lightroom, Bridge, or
older versions of Magpie), or Windows Explorer keywords set through
the *Properties → Details* tab, Magpie **reads them once** on the
first scan and imports them into `magpie.db`. From that point on
the database is the source of truth — later edits in Magpie stay in
the database, and Magpie does **not** try to keep the file's
embedded tags in sync.

If you also edit tags in Explorer or Lightroom, Magpie won't see
those edits until you rescan the folder and manually reimport, so
pick one tool as your master. See
[Interoperability with other tools](./interop.md).

## Folders on OneDrive, Dropbox, or a network share

You can put a Magpie folder on any drive, including cloud-synced
locations like OneDrive, Dropbox, Google Drive, iCloud Drive, or a
network share. The database file itself lives in your local
`AppData\Roaming` folder (which Windows doesn't sync by default), so
sync clients aren't racing Magpie for the database.

If you use Magpie on two PCs, each PC has its own separate
`magpie.db`. Tag edits made on one PC don't appear on the other.

## Magpie's own workshop

Everything Magpie needs lives here:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\
```

That folder has:

- **`magpie.db`** — the database described above. This is where
  your tags and titles are.
- Small **thumbnail images** so the grid loads without delay
  (`thumbs\` subfolder).
- A **log file** that helps if you ever run into trouble
  (`logs\app.log`).

## The three golden rules

1. **Magpie never moves or modifies your original files.** They stay
   byte-for-byte the way they were.
2. **Magpie never uploads anything.** No internet, no cloud, no
   account.
3. **All your tags and titles are in one database.** Copy
   `magpie.db` and you've copied everything Magpie knows.

## Backing up

Back up your `magpie.db` file (or the entire
`%APPDATA%\com.magpie.app\` folder to also keep thumbnails and
logs). That's your whole Magpie library in one file.

Your original photo folders don't need special backup treatment —
they don't contain any Magpie data. Copying them elsewhere copies
only the pixels, not the tags. If you move to a new PC and want
your tags to come with you, copy `magpie.db` too.
