# Working with other tools

Magpie now keeps your tags in a database inside the folder, not
inside the files themselves. That has two implications for other
tools:

- **The files themselves are never modified.** Every viewer and
  editor on your PC sees exactly the bytes the camera (or Photoshop,
  or Lightroom) originally produced.
- **Tags don't automatically sync between Magpie and Explorer /
  Lightroom.** If you want them to end up in the file, you'll need
  to export or write them out with the other tool as before —
  Magpie won't do that automatically.

## Importing existing tags

If a photo already has tags in it (added earlier by Lightroom,
Bridge, or the *Properties → Details* dialog in Explorer, or an
older version of Magpie), Magpie **reads them once** on the first
scan of that folder and imports them into the folder's database.

- Filename tags, XMP `dc:subject`, and Windows Shell `System.Keywords`
  are all recognised.
- The imported title comes from XMP `dc:title` or `System.Title`.

After that first-scan import, Magpie treats its database as the
truth. Later edits you make in Lightroom, Bridge, or Explorer will
**not** show up in Magpie until you rescan that folder.

## Refreshing after external edits

If you know a file was retagged outside Magpie and want to bring
those tags back in:

1. Delete the folder's `.magpie\library.db` file (or rename it).
2. Add the folder to Magpie again — first-scan import will run and
   pick up the current XMP / Explorer keywords.

Any tags you'd already added inside Magpie for that folder will be
lost by this step. Use it only if the folder was tagged elsewhere.

## In short

- Magpie doesn't fight other tools for control of your files.
- If Magpie is your primary tagging tool, everything just works and
  the tags travel with the folder.
- If your other tool is primary and you want its tags in Magpie,
  rescan (or re-import) that folder from time to time.
