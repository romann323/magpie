# Searching and filtering

Once you've added tags and titles to a good number of files, finding
them again is a treat. Magpie combines two kinds of filter — **tag
chips** you pick from the sidebar and **free-text search** you type
in the box — and the grid updates as you go.

## Search + tag chips: how they combine

The search box at the top of the window is really two things:

- **Chips** on the left, one per tag you've ticked in the sidebar.
  Every chip narrows the grid with **AND** logic — a file has to have
  *all* the ticked tags to appear.
- **Free text** on the right, which matches the file's title,
  filename, or any of its tags.

You can mix them freely. For example: tick `beach` and `2023` in the
sidebar (two chips appear), then type `family` in the box, and the
grid shows every file tagged both `beach` **and** `2023` whose title,
filename, or tag also matches `family`.

## Ticking, un-ticking, and clearing tags

- **Tick a tag** in the sidebar to add its chip to the search box.
- **Untick** it, or click the chip's × in the search box, to remove
  it from the search.
- **Clear all** in the sidebar's Tags header un-ticks every tag in
  one click.
- **Backspace** in the empty search box removes the rightmost chip.

## What the free-text search matches

The typed text is matched against three places on every file:

- The **title** you gave it.
- The **tags** attached to it.
- The **filename** (like `IMG_4523.jpg` or `contract_signed.pdf`).

Capital letters don't matter: `Beach` and `beach` find the same
files.

## Handy search tricks (typed text)

- **`beach vacation`** — finds files with **both** words somewhere.
- **`beach OR mountain`** — finds files with **either** word.
- **`vacation -beach`** — vacation files that are **not** tagged
  beach.
- **`"family reunion"`** — the exact phrase, in that order.

Prefix matching is on for the last word — `sun` matches `sunset`,
`sunny`, `sunbathing`.

## Combining with folders

Clicking a folder in the sidebar narrows the grid to files inside
that folder. It works together with tags and search — click a folder,
tick some tags, type some text, and you get the intersection of all
three.

Click **All photos** at the top of the sidebar to remove the folder
filter without losing your tag chips.

## Nothing found?

If a search turns up empty and you were expecting results:

- Check for a typo. `caht` won't find `cat` files.
- Try a shorter word. `mount` finds both `mountain` and `mountains`.
- Look at the tag chips — if you have several ticked, you're asking
  Magpie for files matching **all** of them. Untick one at a time or
  use **Clear all** to widen the search.
