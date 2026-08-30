import { useMutation, useQueryClient } from '@tanstack/react-query'
import { updateAppSettings } from '../ipc'
import { useStore } from '../store'
import type { AppSettings, FontSize, Theme } from '../types'

type Which = 'theme' | 'font-size' | 'language' | null

type Props = {
  which: Which
  onClose: () => void
}

/**
 * Settings modals. All three items in the Settings menu open a modal
 * from here; the parent (`App.tsx`) tracks which one via a piece of
 * component state.
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
    mutationFn: (patch: Partial<Pick<AppSettings, 'theme' | 'fontSize' | 'language'>>) =>
      updateAppSettings(patch),
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
