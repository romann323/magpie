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
 */
export type ImageDetails = ImageSummary & {
  tags: string[]
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
