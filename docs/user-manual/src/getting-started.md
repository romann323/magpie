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

When you first start Magpie you'll see a friendly welcome screen with
two buttons in the middle of the window:

- **New Project…** — creates a fresh empty project. You pick where
  to save it and what to call it (e.g. `Documents\Family.magpie`).
- **Open Project…** — opens an existing `.magpie` file.

Pick **New Project**, save it somewhere sensible, and Magpie opens
into its normal three-panel layout. From then on, that project
re-opens automatically every time you launch. See
[Projects](./projects.md) for more.

Once the project is open the next step is to
[add a folder](./adding-folder.md) full of files to scan.

## What Magpie creates on your computer

Magpie keeps a small workshop of its own in a hidden Windows folder:

```
C:\Users\<you>\AppData\Roaming\com.magpie.app\
```

That folder contains Magpie's preferences (`app-settings.json`, which
remembers your recent projects and theme choices) and small thumbnail
previews. **None of your original files live there** — they stay
exactly where you put them — and your **project files live wherever
you saved them**.

## Uninstalling

- Installed with an `.msi`? Open **Settings › Apps › Installed apps**,
  find Magpie, and click **Uninstall**.
- Portable version? Just delete `desktop.exe`.

To also wipe Magpie's cache and preferences, delete the hidden folder
above (in File Explorer, paste `%APPDATA%\com.magpie.app` into the
address bar). **Your project files aren't in there** unless you chose
that location for them — they stay wherever you saved them.
