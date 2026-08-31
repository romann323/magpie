// Types that mirror the Rust structs in src-tauri/src/types.rs.
// serde converts to camelCase for JS, so keep these keys camelCase.

export type LibraryFolder = {
  id: number
  path: string
  addedAt: number
  lastScanAt: number | null
  imageCount: number
  /**
   * `false` when the folder root can't be reached on disk (removable
   * drive unplugged, network share unreachable, ...). The folder still
   * appears in the sidebar so the user can rescan when the drive is
   * available again.
   */
  isAvailable: boolean
}

/**
 * One row in the grid. `id` is the plain autoincrement primary key of
 * the `images` table — unique across every registered folder because
 * there's only one central DB.
 */
export type ImageSummary = {
  id: number
  folderId: number
  path: string
  filename: string
  ext: string
  width: number | null
  height: number | null
  sizeBytes: number
  mtimeMs: number
  takenAt: number | null
  title: string | null
  contentHash: string | null
}

/**
 * `technical` is an ordered list of `[label, value]` pairs contributed by the
 * format handler for the current file (dimensions, EXIF, camera, duration, ...).
 * The list is display-ready; UI just prints it verbatim.
 *
 * `formatHandler` is the human-readable handler name (e.g. `"jpeg"`) and is
 * shown in the "Format metadata" section of the details panel.
 *
 * `importedAt` is the timestamp (ms since epoch) the row was first added to
 * the central Magpie DB.
 *
 * Tags are split by provenance so the right pane can show them
 * distinctly:
 * - `userTags` — added by the user inside Magpie, editable.
 * - `autoTags` — imported from the file's own metadata (XMP subjects,
 *   Windows Shell keywords, sidecar XMP) at scan time, read-only.
 *
 * The same name can appear in both lists if both sources carry it.
 */
export type ImageDetails = ImageSummary & {
  userTags: string[]
  autoTags: string[]
  technical: Array<[string, string]>
  formatHandler: string
  importedAt: number
}

export type SortBy = 'takenAt' | 'filename' | 'addedAt' | 'size'
export type SortDir = 'asc' | 'desc'

export type ImageFilter = {
  folderIds?: number[]
  tagsAny?: string[]
  tagsAll?: string[]
  tagsNone?: string[]
  takenAfter?: number
  takenBefore?: number
  ext?: string[]
  fts?: string
  hasTitle?: boolean
}

export type ImageSort = { by: SortBy; dir: SortDir }

export type Pagination = { offset: number; limit: number }

export type Page<T> = {
  items: T[]
  total: number
  offset: number
  limit: number
}

export type MetadataPatch = {
  // The double-optional pattern matches Rust's Option<Option<T>>:
  // - undefined  → don't touch the field
  // - null       → clear the field
  // - value      → set the field
  title?: string | null
  tags?: string[]
  tagsAdd?: string[]
  tagsRemove?: string[]
}

export type TagStats = { name: string; count: number }

export type ScanProgress = {
  folderId: number
  processed: number
  total: number
  currentPath: string | null
  finished: boolean
}

export type ScanResult = {
  folderId: number
  added: number
  updated: number
  removed: number
  errors: number
}

/**
 * Progress payload of `app://auto-tag`. Mirrors `ScanProgress` and is
 * emitted by the automatic-AI-tagging pipeline as it works through a
 * folder that was just added / rescanned.
 */
export type AutoTagProgress = {
  folderId: number
  processed: number
  total: number
  currentPath: string | null
  /** Cumulative count of tags this run has attached across all images so far. */
  tagsAdded: number
  /** Number of images this run skipped because they were already tagged and unchanged. */
  skipped: number
  finished: boolean
  /**
   * Populated only on the terminal event (finished=true) when the run
   * short-circuited without processing anything — typically because
   * the AI model files aren't downloaded yet. Surfaced as a warning
   * strip in the status bar.
   */
  error?: string | null
}

/**
 * Snapshot of the on-disk CLIP model cache, returned by
 * `checkAiModelStatus()`. `ready` is the single boolean the UI
 * should key off — every other field is diagnostic.
 */
export type AiModelStatus = {
  ready: boolean
  /** True once `model.safetensors` is on disk and passes the pinned checksum. */
  modelPresent: boolean
  tokenizerPresent: boolean
  embeddingsPresent: boolean
  totalBytes: number
  bytesOnDisk: number
}

/**
 * Payload of `app://ai-model-download`, emitted every ~200 ms while
 * a `downloadAiModel()` call is in flight. `finished=true` marks the
 * terminal event; `error` is populated only on failure.
 */
export type AiModelDownloadProgress = {
  currentFile: string
  currentBytes: number
  currentTotal: number
  totalBytes: number
  totalExpected: number
  finished: boolean
  error?: string | null
}

export type SmartCollection = {
  id: number
  name: string
  filter: ImageFilter
  sortOrder: number
}

export type DeleteFailure = {
  id: number
  path: string
  error: string
}

export type DeleteResult = {
  deleted: number[]
  failed: DeleteFailure[]
}

export type ThumbSize = 'small' | 'medium' | 'large'

export type AppError = {
  code: string
  message: string
}

// ---------- Projects & app settings ----------

export type ProjectInfo = {
  path: string
  name: string
}

export type Theme = 'system' | 'dark' | 'light'
export type FontSize = 'small' | 'medium' | 'large'

export type AppSettings = {
  theme: Theme
  fontSize: FontSize
  language: string
  lastProjectPath: string | null
  recentProjects: string[]
  /**
   * When true, Magpie automatically runs AI-based tag assignment on
   * every image in a library folder immediately after the folder's
   * filesystem scan finishes. Toggled via **Settings → Auto-tag
   * photos**.
   */
  aiAutoTag: boolean
}

export type AppSettingsPatch = Partial<
  Pick<AppSettings, 'theme' | 'fontSize' | 'language' | 'aiAutoTag'>
>

// ---------- Menu ----------

/** Payload of the `app://menu` event: the ID of the clicked item. */
export type MenuEventId = string

// ---------- Magnifier ----------

/**
 * Context handed from the main window to the Magnifier popup: which
 * image to show first, plus the filter and sort of the list the
 * magnifier should walk when the user presses ← / →.
 */
export type MagnifierContext = {
  imageId: number | null
  filter: ImageFilter
  sort: ImageSort
}
