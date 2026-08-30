import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AppSettings,
  AppSettingsPatch,
  AutoTagProgress,
  DeleteResult,
  ImageDetails,
  ImageFilter,
  ImageSort,
  ImageSummary,
  LibraryFolder,
  MagnifierContext,
  MenuEventId,
  MetadataPatch,
  Page,
  Pagination,
  ProjectInfo,
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

export const renameImage = (id: number, newFilename: string) =>
  invoke<ImageDetails>('rename_image', { id, newFilename })

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

// ---------- Projects ----------

export const currentProject = () =>
  invoke<ProjectInfo | null>('current_project')

export const createProject = (path: string) =>
  invoke<ProjectInfo>('create_project', { path })

export const openProject = (path: string) =>
  invoke<ProjectInfo>('open_project', { path })

export const saveProject = () => invoke<ProjectInfo>('save_project')

export const saveProjectAs = (path: string) =>
  invoke<ProjectInfo>('save_project_as', { path })

export const closeProject = () => invoke<void>('close_project')

// ---------- App settings ----------

export const getAppSettings = () => invoke<AppSettings>('get_app_settings')

export const updateAppSettings = (patch: AppSettingsPatch) =>
  invoke<AppSettings>('update_app_settings', { patch })

// ---------- Menu control ----------

export const setMenuItemEnabled = (id: string, enabled: boolean) =>
  invoke<void>('set_menu_item_enabled', { id, enabled })

export const setMenuItemLabel = (id: string, label: string) =>
  invoke<void>('set_menu_item_label', { id, label })

// ---------- Magnifier context ----------

export const getMagnifierContext = () =>
  invoke<MagnifierContext>('get_magnifier_context')

export const setMagnifierContext = (
  imageId: number | null,
  filter: ImageFilter,
  sort: ImageSort,
) => invoke<void>('set_magnifier_context', { imageId, filter, sort })

export const setMagnifierCurrent = (imageId: number | null) =>
  invoke<void>('set_magnifier_current', { imageId })

// ---------- Events ----------

export const onScanProgress = (
  handler: (p: ScanProgress) => void,
): Promise<UnlistenFn> => listen<ScanProgress>('app://scan', (e) => handler(e.payload))

export const onAutoTagProgress = (
  handler: (p: AutoTagProgress) => void,
): Promise<UnlistenFn> =>
  listen<AutoTagProgress>('app://auto-tag', (e) => handler(e.payload))

export const onImageUpdated = (
  handler: (id: number) => void,
): Promise<UnlistenFn> => listen<number>('app://image-updated', (e) => handler(e.payload))

export const onImagesDeleted = (
  handler: (ids: number[]) => void,
): Promise<UnlistenFn> =>
  listen<number[]>('app://images-deleted', (e) => handler(e.payload))

export const onMenuEvent = (
  handler: (id: MenuEventId) => void,
): Promise<UnlistenFn> => listen<MenuEventId>('app://menu', (e) => handler(e.payload))

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
