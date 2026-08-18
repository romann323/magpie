# Installing and launching PicOrg

## System requirements

- **Operating system:** Windows 10 (build 19041) or newer, or Windows 11.
  Requires the Microsoft Edge WebView2 runtime, which is pre-installed
  on Windows 11 and delivered automatically by Windows Update on
  Windows 10.
- **Disk:** ~50 MB for the app itself, plus roughly **8 KB per
  thumbnail** (two sizes per photo) in the cache. A 50 000-photo
  library uses ~400 MB of thumbnails.
- **Memory:** 200 MB idle; scales with library size and grid density.

## Installing

Grab the latest release from your usual distribution channel (installer
`.msi`, portable `.exe`, or self-built from source), then double-click
to launch. PicOrg is a single-window app — no service to start, no
account to create.

> **Note.** PicOrg does not need administrator privileges. If Windows
> prompts you for elevation, that means you're installing a signed MSI
> — say "yes" once, then run the app as your normal user.

## First launch

The first time you run PicOrg it creates:

- The library database at
  `%APPDATA%\com.picorg.picorg\picorg.db`.
- An empty thumbnail cache at
  `%APPDATA%\com.picorg.picorg\thumbs\`.
- A log file at
  `%APPDATA%\com.picorg.picorg\logs\picorg.log`.

The main window opens on **All photos** with a helpful "Add folder"
prompt. See the next chapter to add your first folder.

## Where your library data lives

| What                                | Path                                                  |
| ----------------------------------- | ----------------------------------------------------- |
| Library database                    | `%APPDATA%\com.picorg.picorg\picorg.db`               |
| Thumbnail cache                     | `%APPDATA%\com.picorg.picorg\thumbs\`                 |
| Log file                            | `%APPDATA%\com.picorg.picorg\logs\picorg.log`         |
| **Your photos**                     | Wherever you put them — PicOrg never moves them       |
| **Sidecar `.xmp` files**            | Next to each photo (`Photo.jpg` → `Photo.xmp`)        |
| **Embedded XMP** (JPEG only, v1)    | Inside the JPEG APP1 segment of the source file       |

## Uninstalling

Delete `%APPDATA%\com.picorg.picorg\` to wipe the library index and
cache. That resets PicOrg completely but leaves every photo and every
sidecar `.xmp` on disk exactly as it was.
