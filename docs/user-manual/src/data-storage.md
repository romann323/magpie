# Where your data lives

A quick, plain-language tour of what Magpie puts where. You don't
need to know any of this to use the app — but many people like
knowing exactly where their information ends up.

## Your files

**Your files stay where you put them.** Magpie never moves them, and
never modifies their contents. The only thing Magpie can change on
disk is a file's **name**, and only when you explicitly rename it in
the details panel.

**Magpie never writes tags into your files.** Older versions of
Magpie tried to embed tags directly into JPEGs and PNGs. That
approach caused problems (files got modified even when the user was
just browsing, some formats couldn't be written to at all, cloud
sync services would re-upload gigabytes for a one-word tag change).
Magpie now leaves file contents completely untouched.

## Your tags and titles: the project file

Tags, titles, folder lists, and smart searches all live inside
**your project file** — a single `.magpie` file you chose the
location and name of. For example:

```
C:\Users\<you>\Documents\Family.magpie
```

That single file remembers:

- Every folder you've added to the project.
- Every image Magpie has seen in each folder.
- The tags and title you've set for each image.
- A search index so typing in the search box is instant.
- Any smart searches (saved searches) you've created.

Because it's just a file, you can:

- **Back it up** by copying it somewhere safe.
- **Share it** by emailing or moving it to another PC. On the other
  PC, open Magpie → **Project → Open Project…** and pick your copy.
- **Archive it** by moving it into a `.zip`.

**Every file becomes taggable.** RAW files, videos, PDFs, HEIC,
TIFF, and every other format are all tagged the same way now,
because the tags are in your project file, not in the file.

## Multiple projects

You can have as many project files as you like — for example one for
family photos and another for work documents. Magpie can only have
**one open at a time**; switch between them from the **Project**
menu. See [Projects](./projects.md).

## Existing tags in your files

If your photos already carry XMP tags (from Lightroom, Bridge, or
older versions of Magpie), or Windows Explorer keywords set through
the *Properties → Details* tab, Magpie **reads them once** on the
first scan and imports them into your project. From that point on
the project file is the source of truth — later edits in Magpie stay
in the project, and Magpie does **not** try to keep the file's
embedded tags in sync.

If you also edit tags in Explorer or Lightroom, Magpie won't see
those edits until you rescan the folder and manually reimport, so
pick one tool as your master. See
[Interoperability with other tools](./interop.md).

## Folders on OneDrive, Dropbox, or a network share

You can point Magpie at any folder on any drive, including
cloud-synced locations like OneDrive, Dropbox, Google Drive, iCloud
Drive, or a network share. The project file can live wherever you
want too — including a synced location if you want your tags to
follow you between PCs.

## Magpie's own workshop

Even though your project lives wherever you saved it, Magpie also
keeps a small workshop of its own in a hidden Windows folder:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\
```

That folder has:

- **`app-settings.json`** — remembers which project was last open,
  your recent-projects list, and your theme / font-size / language
  choices.
- **`Default.magpie`** — only present if Magpie converted an
  older-format library into a project on your behalf. It's just a
  normal project file; move it wherever you like.
- Small **thumbnail images** so the grid loads without delay
  (`thumbs\` subfolder). Thumbnails are stored in a separate
  subdirectory for each project so switching between projects
  never shows you a preview from the wrong one.
- A **log file** that helps if you ever run into trouble
  (`logs\app.log`).

None of your projects have to live here — you're free to save them
wherever you want.

## The three golden rules

1. **Magpie never modifies file contents.** It never uploads them,
   and it can only rename them when you explicitly do so.
2. **Magpie never uploads anything.** No internet, no cloud, no
   account.
3. **All your tags and titles are in your project file.** Copy the
   `.magpie` file and you've copied everything Magpie knows about
   that project.

## Backing up

Back up your `.magpie` file(s). That's your whole Magpie library
in one file per project.

Your original photo folders don't need special backup treatment —
they don't contain any Magpie data. Copying them elsewhere copies
only the pixels, not the tags. If you move to a new PC and want
your tags to come with you, copy the `.magpie` file(s) too.
