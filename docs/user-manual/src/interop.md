# Working with other tools

Magpie keeps your tags in its own database, not inside the files
themselves. That has two implications for other tools:

- **The files themselves are never modified.** Every viewer and
  editor on your PC sees exactly the bytes the camera (or Photoshop,
  or Lightroom) originally produced.
- **Tags don't automatically sync between Magpie and Explorer /
  Lightroom.** If you want them to end up in the file, you'll need
  to write them out with the other tool as before — Magpie won't do
  that automatically.

## Importing existing tags

If a photo already has tags in it (added earlier by Lightroom,
Bridge, or the *Properties → Details* dialog in Explorer, or an
older version of Magpie), Magpie **reads them once** on the first
scan of that folder and imports them into its database.

- XMP `dc:subject`, Microsoft-Photo `MP:LastKeywordXMP`, and Windows
  Shell `System.Keywords` are all recognised.
- The imported title comes from XMP `dc:title` or `System.Title`.

After that first-scan import, Magpie treats its database as the
truth. Later edits you make in Lightroom, Bridge, or Explorer will
**not** show up in Magpie until you re-import the folder.

## Refreshing after external edits

If you know a file was retagged outside Magpie and want to bring
those tags back in, the simplest path is:

1. **Remove** the folder from Magpie (right-click it in the sidebar →
   *Remove from library*). Your files aren't touched.
2. **Add** the folder again. The first-scan import runs and picks
   up the current XMP / Explorer keywords for every file.

Anything you'd added *only* inside Magpie for that folder will be
lost by this step, because Magpie doesn't write tags back into the
files. Use it only when the external tool is the authoritative
source.

## In short

- Magpie doesn't fight other tools for control of your files.
- If Magpie is your primary tagging tool, everything just works.
- If your other tool is primary and you want its tags in Magpie,
  re-import that folder from time to time.
