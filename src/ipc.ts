import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  DeleteResult,
  ImageDetails,
  ImageFilter,
  ImageSort,
  ImageSummary,
  LibraryFolder,
  MetadataPatch,
  Page,
  Pagination,
  ScanProgress,
  ScanResult,
  SmartCollection,
  TagStats,
  ThumbSize,
} from './types'

// ---------- Library ----------

export const addLibraryFolder = (path: string) =>
  invoke<LibraryFolder>('add_library_folder', { path })

export const removeLibraryFolder = (id: number) =>
  invoke<void>('remove_library_folder', { id })

export const listLibraryFolders = () =>
  invoke<LibraryFolder[]>('list_library_folders')

export const rescanFolder = (id: number) =>
  invoke<ScanResult>('rescan_folder', { id })

export const rescanAll = () => invoke<ScanResult[]>('rescan_all')

// ---------- Images ----------

export const queryImages = (params: {
  filter?: ImageFilter
  sort?: ImageSort
  page?: Pagination
}) => invoke<Page<ImageSummary>>('query_images', params)

export const getImage = (id: number) =>
  invoke<ImageDetails>('get_image', { id })

export const updateImageMetadata = (id: number, patch: MetadataPatch) =>
  invoke<ImageDetails>('update_image_metadata', { id, patch })

export const batchUpdateMetadata = (ids: number[], patch: MetadataPatch) =>
  invoke<number[]>('batch_update_metadata', { ids, patch })

export const deleteImages = (ids: number[], permanent = false) =>
  invoke<DeleteResult>('delete_images', { ids, permanent })

// ---------- Tags ----------

export const listTags = (prefix?: string) =>
  invoke<TagStats[]>('list_tags', { prefix })

export const renameTag = (oldName: string, newName: string) =>
  invoke<void>('rename_tag', { oldName, newName })

export const deleteTag = (name: string) =>
  invoke<void>('delete_tag', { name })

// ---------- Smart collections ----------

export const listSmartCollections = () =>
  invoke<SmartCollection[]>('list_smart_collections')

export const createSmartCollection = (name: string, filter: ImageFilter) =>
  invoke<SmartCollection>('create_smart_collection', { name, filter })

export const deleteSmartCollection = (id: number) =>
  invoke<void>('delete_smart_collection', { id })

// ---------- Thumbnails / images ----------

export const getThumbPath = (id: number, size: ThumbSize = 'small') =>
  invoke<string>('get_thumb_path', { id, size })

export const getImagePath = (id: number) =>
  invoke<string>('get_image_path', { id })

/** Convert an absolute file path to an `asset://` URL usable in <img src>. */
export const toAssetUrl = (absPath: string): string => convertFileSrc(absPath)

// ---------- Events ----------

export const onScanProgress = (
  handler: (p: ScanProgress) => void,
): Promise<UnlistenFn> => listen<ScanProgress>('picorg://scan', (e) => handler(e.payload))

export const onImageUpdated = (
  handler: (id: number) => void,
): Promise<UnlistenFn> => listen<number>('picorg://image-updated', (e) => handler(e.payload))

export const onImagesDeleted = (
  handler: (ids: number[]) => void,
): Promise<UnlistenFn> =>
  listen<number[]>('picorg://images-deleted', (e) => handler(e.payload))

// ---------- Diagnostics ----------

/**
 * Send a log message from the renderer to the Rust log file. Best-effort — a
 * failure is swallowed so telemetry never blocks user-facing UI flows.
 */
export const logFrontend = (
  level: 'debug' | 'info' | 'warn' | 'error',
  msg: string,
): void => {
  void invoke('log_frontend', { level, msg }).catch(() => {})
}
