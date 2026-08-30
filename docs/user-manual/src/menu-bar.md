# The menu bar

Across the top of the window is a standard menu bar with four menus:
**Project**, **Edit**, **View**, and **Settings**. This chapter
lists what each item does.

## Project

Everything about the current project file — see
[Projects](./projects.md) for the full walkthrough.

| Item                | Shortcut          | What it does                          |
| ------------------- | ----------------- | ------------------------------------- |
| **New Project…**    | Ctrl + N          | Create a new project.                 |
| **Open Project…**   | Ctrl + O          | Open an existing project.             |
| **Save Project**    | Ctrl + S          | (Nothing to do — Magpie saves as you type.) |
| **Save Project As…**| Ctrl + Shift + S  | Copy the current project to a new file. |
| **Close Project**   |                   | Close the current project.            |
| **Exit**            | Alt + F4          | Quit Magpie.                          |

## Edit

The **Edit** menu covers session-only undo / redo for the changes you
make with the details panel. Both items are greyed out until you've
actually made a change (or, for Redo, until you've undone one).

| Item     | Shortcut | What it undoes                                    |
| -------- | -------- | ------------------------------------------------- |
| **Undo** | Ctrl + Z | The last title change, tag change, or filename rename. |
| **Redo** | Ctrl + Y | Reapplies whatever you just undid.                |

Undo history is **per session** — it's cleared when you close the
project or quit the app.

## View

| Item          | Shortcut | What it does                                                                 |
| ------------- | -------- | ---------------------------------------------------------------------------- |
| **Magnifier** | F11      | Opens the currently-selected picture full-window. Use ← / → to walk through the grid, Esc to close. Double-clicking a tile in the grid does the same thing. |

## Settings

| Item                  | What it does                                                          |
| --------------------- | --------------------------------------------------------------------- |
| **Language…**         | Currently English-only. (Additional languages are on the roadmap.)    |
| **Theme…**            | Choose Dark, Light, or Follow-system. Your choice is remembered.       |
| **Font size…**        | Small, Medium (default), or Large — scales the whole UI.               |
| **Auto-tag photos**   | Toggle Magpie's built-in tagger. A trailing **✓** means it's on. See [Settings → Auto-tag photos](./settings.md#auto-tag-photos). |

All settings apply immediately and are saved in `%APPDATA%\com.magpie.app\app-settings.json`.
