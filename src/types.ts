// Types that mirror the Rust structs in src-tauri/src/types.rs.
// serde converts to camelCase for JS, so keep these keys camelCase.

export type LibraryFolder = {
  id: number
  path: string
  addedAt: number
  lastScanAt: number | null
  imageCount: number
}

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
 * format handler for the current file (dimensions, EXIF, camera, duration, …).
 * The list is display-ready; UI just prints it verbatim.
 *
 * `formatHandler` is the human-readable handler name (e.g. `"JPEG (XMP APP1)"`)
 * and `canWriteTags` reports whether the handler supports embedding user tags
 * into the source file.
 */
/** How Magpie will persist title / tags for a given file. */
export type WriteMode =
  /** Format handler embeds directly (JPEG XMP, PNG iTXt, WebP, GIF89a, ...). */
  | 'native'
  /** Windows Shell property system (RAW, MP4/MOV, HEIC, TIFF, ...). */
  | 'shell'
  /** No writable path: metadata lives in the Magpie library only. */
  | 'libraryOnly'

export type ImageDetails = ImageSummary & {
  tags: string[]
  cameraMake: string | null
  cameraModel: string | null
  metaWrittenAt: number | null
  metaReadAt: number | null
  technical: Array<[string, string]>
  formatHandler: string
  canWriteTags: boolean
  writeMode: WriteMode
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
