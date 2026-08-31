import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  checkAiModelStatus,
  clearAiModel,
  downloadAiModel,
  onAiModelDownloadProgress,
  updateAppSettings,
} from '../ipc'
import { useStore } from '../store'
import type {
  AiModelDownloadProgress,
  AiModelStatus,
  AppSettings,
  FontSize,
  Theme,
} from '../types'

type Which = 'theme' | 'font-size' | 'language' | 'ai-auto-tag' | null

type Props = {
  which: Which
  onClose: () => void
}

/**
 * Settings modals. All Settings menu items open a modal from here;
 * the parent (`App.tsx`) tracks which one via a piece of component
 * state.
 */
export function SettingsDialogs({ which, onClose }: Props) {
  if (!which) return null

  return (
    <>
      <div className="modal-backdrop" onClick={onClose} />
      <div className="modal-panel">
        {which === 'theme' && <ThemeDialog onClose={onClose} />}
        {which === 'font-size' && <FontSizeDialog onClose={onClose} />}
        {which === 'language' && <LanguageDialog onClose={onClose} />}
        {which === 'ai-auto-tag' && <AiAutoTagDialog onClose={onClose} />}
      </div>
    </>
  )
}

function DialogFrame({
  title,
  onClose,
  children,
}: {
  title: string
  onClose: () => void
  children: React.ReactNode
}) {
  return (
    <div className="modal-card">
      <div className="flex items-center justify-between mb-3">
        <div className="text-sm font-medium">{title}</div>
        <button className="btn-icon" onClick={onClose} title="Close">
          ×
        </button>
      </div>
      {children}
    </div>
  )
}

function usePersistSetting() {
  const qc = useQueryClient()
  const setSettings = useStore((s) => s.setSettings)
  return useMutation({
    mutationFn: (
      patch: Partial<
        Pick<AppSettings, 'theme' | 'fontSize' | 'language' | 'aiAutoTag'>
      >,
    ) => updateAppSettings(patch),
    onSuccess: (settings) => {
      setSettings(settings)
      qc.invalidateQueries({ queryKey: ['app-settings'] })
    },
  })
}

function ThemeDialog({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings)
  const persist = usePersistSetting()
  const current: Theme = settings?.theme ?? 'system'

  const options: { value: Theme; label: string; note?: string }[] = [
    { value: 'system', label: 'Follow system', note: 'Match Windows' },
    { value: 'dark', label: 'Dark' },
    { value: 'light', label: 'Light (preview)' },
  ]

  return (
    <DialogFrame title="Theme" onClose={onClose}>
      <div className="flex flex-col gap-2">
        {options.map((o) => (
          <label
            key={o.value}
            className="flex items-center gap-2 p-2 rounded hover:bg-surface-hover cursor-pointer"
          >
            <input
              type="radio"
              name="theme"
              checked={current === o.value}
              onChange={() => persist.mutate({ theme: o.value })}
            />
            <span className="text-sm">{o.label}</span>
            {o.note && <span className="text-[11px] text-slate-500">— {o.note}</span>}
          </label>
        ))}
      </div>
      <div className="text-[11px] text-slate-500 mt-3">
        The light theme is a preview and may look off in some spots. Please
        report any places you'd like polished.
      </div>
    </DialogFrame>
  )
}

function FontSizeDialog({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings)
  const persist = usePersistSetting()
  const current: FontSize = settings?.fontSize ?? 'medium'

  const options: { value: FontSize; label: string; note: string }[] = [
    { value: 'small', label: 'Small', note: 'Compact — fits more per screen' },
    { value: 'medium', label: 'Medium', note: 'Default' },
    { value: 'large', label: 'Large', note: 'Easier to read' },
  ]

  return (
    <DialogFrame title="Font size" onClose={onClose}>
      <div className="flex flex-col gap-2">
        {options.map((o) => (
          <label
            key={o.value}
            className="flex items-center gap-2 p-2 rounded hover:bg-surface-hover cursor-pointer"
          >
            <input
              type="radio"
              name="font-size"
              checked={current === o.value}
              onChange={() => persist.mutate({ fontSize: o.value })}
            />
            <span className="text-sm">{o.label}</span>
            <span className="text-[11px] text-slate-500">— {o.note}</span>
          </label>
        ))}
      </div>
    </DialogFrame>
  )
}

function LanguageDialog({ onClose }: { onClose: () => void }) {
  return (
    <DialogFrame title="Language" onClose={onClose}>
      <div className="text-sm text-slate-300">
        Magpie is currently English-only. Additional languages are coming in a
        future release.
      </div>
      <div className="flex justify-end mt-4">
        <button className="btn" onClick={onClose}>
          OK
        </button>
      </div>
    </DialogFrame>
  )
}

// ---------------------------------------------------------------------
//                       Auto-tag photos dialog
// ---------------------------------------------------------------------

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

function AiAutoTagDialog({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings)
  const persist = usePersistSetting()
  const qc = useQueryClient()

  // Poll status on mount and on every download progress tick so the
  // "Ready" banner flips as soon as the last file lands.
  const statusQ = useQuery({
    queryKey: ['ai-model-status'],
    queryFn: checkAiModelStatus,
    // The status is cheap (stat calls) but we don't need to hit it
    // constantly — every 5 s is plenty when idle.
    refetchInterval: 5_000,
  })
  const status: AiModelStatus | undefined = statusQ.data

  const [downloadProgress, setDownloadProgress] =
    useState<AiModelDownloadProgress | null>(null)
  const [downloading, setDownloading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let un: (() => void) | null = null
    onAiModelDownloadProgress((p) => {
      setDownloadProgress(p)
      if (p.finished) {
        setDownloading(false)
        setError(p.error ?? null)
        qc.invalidateQueries({ queryKey: ['ai-model-status'] })
      }
    })
      .then((fn) => {
        un = fn
      })
      .catch(() => {})
    return () => {
      if (un) un()
    }
  }, [qc])

  const startDownload = async () => {
    setError(null)
    setDownloading(true)
    setDownloadProgress(null)
    try {
      await downloadAiModel()
    } catch (e) {
      setError((e as Error).message)
      setDownloading(false)
    }
  }

  const clearModel = async () => {
    if (
      !confirm(
        'Delete downloaded AI model files? Auto-tagging will need to re-download them next time you enable it.',
      )
    ) {
      return
    }
    try {
      // Also turn off auto-tag so the next scan doesn't try to run
      // without a model.
      if (settings?.aiAutoTag) {
        await persist.mutateAsync({ aiAutoTag: false })
      }
      await clearAiModel()
      qc.invalidateQueries({ queryKey: ['ai-model-status'] })
    } catch (e) {
      alert(`Could not clear model: ${(e as Error).message}`)
    }
  }

  const ready = status?.ready === true
  const enabled = settings?.aiAutoTag === true

  const toggleEnable = () => {
    if (!ready) {
      // Refuse to enable when the model isn't downloaded — the toggle
      // stays disabled until the download button has completed.
      return
    }
    persist.mutate({ aiAutoTag: !enabled })
  }

  return (
    <DialogFrame title="Auto-tag photos" onClose={onClose}>
      <div className="text-[13px] text-slate-300 mb-3 leading-relaxed">
        Automatically add tags to every photo in a library folder based on what
        the image actually contains — using an on-device CLIP model. Nothing
        is sent to any online service; everything runs on your PC.
      </div>

      {/* --- Model status --- */}
      <div className="mb-4">
        <div className="text-xs uppercase tracking-wide text-slate-400 mb-2">
          AI model
        </div>
        {status ? (
          <div className="border border-slate-700 rounded p-3 bg-slate-900/40">
            <div className="flex items-center justify-between mb-2">
              <div className="text-sm">
                {ready ? (
                  <span className="text-emerald-400">✓ Model ready</span>
                ) : (
                  <span className="text-amber-400">Not downloaded</span>
                )}
              </div>
              <div className="text-[11px] text-slate-500">
                {formatBytes(status.bytesOnDisk)} / {formatBytes(status.totalBytes)}
              </div>
            </div>

            {downloading && downloadProgress && (
              <div className="mb-2">
                <div className="text-[11px] text-slate-400 mb-1 truncate">
                  {downloadProgress.currentFile
                    ? `Downloading ${downloadProgress.currentFile}...`
                    : 'Preparing...'}
                </div>
                <div className="h-1 w-full bg-slate-800 rounded overflow-hidden">
                  <div
                    className="h-full bg-emerald-500 transition-all"
                    style={{
                      width: `${Math.min(
                        100,
                        (downloadProgress.totalBytes /
                          Math.max(1, downloadProgress.totalExpected)) *
                          100,
                      )}%`,
                    }}
                  />
                </div>
                <div className="text-[11px] text-slate-500 mt-1">
                  {formatBytes(downloadProgress.totalBytes)} /{' '}
                  {formatBytes(downloadProgress.totalExpected)}
                </div>
              </div>
            )}

            {error && (
              <div className="text-[12px] text-red-400 mt-1 mb-2">
                Error: {error}
              </div>
            )}

            <div className="flex gap-2 mt-2">
              {!ready && !downloading && (
                <button
                  className="btn btn-primary"
                  onClick={() => void startDownload()}
                  disabled={downloading}
                >
                  Download AI model ({formatBytes(status.totalBytes)})
                </button>
              )}
              {ready && (
                <button
                  className="btn"
                  onClick={() => void clearModel()}
                  disabled={downloading}
                >
                  Remove model files
                </button>
              )}
            </div>
            <div className="text-[11px] text-slate-500 mt-2">
              Files are stored under your app data folder and never leave
              your PC.
            </div>
          </div>
        ) : (
          <div className="text-[12px] text-slate-500">Checking status...</div>
        )}
      </div>

      {/* --- Enable toggle --- */}
      <label
        className={`flex items-center gap-2 p-2 rounded ${
          ready
            ? 'hover:bg-surface-hover cursor-pointer'
            : 'opacity-50 cursor-not-allowed'
        }`}
      >
        <input
          type="checkbox"
          checked={enabled}
          onChange={toggleEnable}
          disabled={!ready}
        />
        <span className="text-sm">
          Automatically tag photos when adding a new folder
        </span>
      </label>
      {!ready && (
        <div className="text-[11px] text-slate-500 pl-6">
          Download the AI model first to enable this option.
        </div>
      )}

      <div className="flex justify-end mt-4">
        <button className="btn" onClick={onClose}>
          OK
        </button>
      </div>
    </DialogFrame>
  )
}
