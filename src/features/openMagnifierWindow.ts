import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { setMagnifierContext } from '../ipc'
import type { ImageFilter, ImageSort } from '../types'

/**
 * Open (or focus) the standalone Magnifier popup window and hand it a
 * fresh context: which image to display first + the filter/sort of the
 * list it should navigate.
 *
 * The context is stashed in the Rust process rather than the URL so we
 * don't have to serialise `ImageFilter` / `ImageSort` into a query
 * string. The popup pulls it in with `get_magnifier_context()` on
 * mount.
 */
export const MAGNIFIER_WINDOW_LABEL = 'magnifier'

export async function openMagnifierWindow(
  imageId: number,
  filter: ImageFilter,
  sort: ImageSort,
): Promise<void> {
  await setMagnifierContext(imageId, filter, sort)

  const existing = await WebviewWindow.getByLabel(MAGNIFIER_WINDOW_LABEL)
  if (existing) {
    try {
      await existing.setFocus()
      // Force the popup to re-read the freshly-stashed context.
      await existing.emit('app://magnifier-reset', imageId)
    } catch {
      // The window may have been closed between getByLabel() and now;
      // fall through to creating a new one.
    }
    return
  }

  // The URL must start with `/` — Tauri appends it to the base URL
  // (`tauri://localhost/` in production, the Vite dev URL in dev).
  // The `#magnifier` fragment is what `main.tsx` looks at to decide
  // to mount `<MagnifierWindow />` instead of `<App />`.
  const win = new WebviewWindow(MAGNIFIER_WINDOW_LABEL, {
    url: '/index.html#magnifier',
    title: 'Magnifier',
    width: 1200,
    height: 800,
    minWidth: 480,
    minHeight: 320,
    resizable: true,
    decorations: true,
    center: true,
    focus: true,
    devtools: true,
  })

  win.once('tauri://error', (event) => {
    // Surface creation errors to the console so we can see them in dev
    // tools; the main window is unaffected either way.
    // eslint-disable-next-line no-console
    console.error('Failed to open Magnifier window:', event.payload)
  })
}
