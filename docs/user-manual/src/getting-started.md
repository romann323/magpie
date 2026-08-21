# Installing and launching Magpie

## What you need

- A PC running **Windows 10** or **Windows 11**.
- About **50 MB of free space** for the app.
- A little extra space for small thumbnail images (roughly 100 MB for
  every 10,000 files).

That's it. No account, no internet connection, no subscription.

## Installing

Depending on where you got Magpie from, you'll have one of these:

- **An installer** (a file ending in `.msi` or `.exe`). Double-click
  it and follow the prompts. When Windows asks whether to allow the
  installer to make changes, click **Yes**.
- **A portable version** (just `desktop.exe`). Copy it wherever you
  like — for example, `C:\Programs\Magpie\` — and double-click it to
  launch. No installation needed.

If Windows shows a blue "Windows protected your PC" screen, click
**More info** and then **Run anyway**. This happens with any small
app that hasn't paid Microsoft for a code signature — it doesn't
mean anything is wrong.

## The very first time you open it

When you first start Magpie you'll see:

- An empty window with a friendly **"Add folder"** button.
- The status bar at the bottom saying **Idle**.

Nothing is scanned yet. Nothing is stored yet. Magpie is waiting for
you to point it at a folder.

The [next chapter](./adding-folder.md) walks you through that in
detail.

## What Magpie creates on your computer

Magpie keeps its own little workshop in a hidden Windows folder. You
don't need to open it or think about it — but if you're curious, it's
here:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\
```

That folder contains Magpie's own database (an index of the files it
knows about) and small thumbnail previews. **None of your original
files live there** — they stay exactly where you put them.

## Uninstalling

- Installed with an `.msi`? Open **Settings › Apps › Installed apps**,
  find Magpie, and click **Uninstall**.
- Portable version? Just delete `desktop.exe`.

To also wipe Magpie's own files, delete the hidden folder above (in
File Explorer, paste `%APPDATA%\com.magpie.app` into the address
bar). **Your files and any tags you added to writable formats are
safe** — those live in your folders, not in Magpie's workshop.
