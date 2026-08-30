import { useEffect, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { onAutoTagProgress, onScanProgress } from '../ipc'
import { PRODUCT_NAME, PRODUCT_VERSION } from '../brand'
import type { AutoTagProgress, ScanProgress } from '../types'

/**
 * Bottom status bar. Shows product build info plus two independent
 * progress lines when background work is running:
 *
 * - `Scanning — N / M` while the filesystem scanner is walking a
 *   folder (event `app://scan`).
 * - `Auto-tagging — N / M` while the automatic-AI-tagging pass is
 *   working through the just-scanned folder (event `app://auto-tag`).
 *
 * Both lines auto-hide 2.5 s after their `finished: true` event fires.
 * When auto-tagging finishes the tag and image query caches are
 * invalidated so the sidebar and details pane pick up the new tags.
 */
export function StatusBar() {
  const [scan, setScan] = useState<ScanProgress | null>(null)
  const [ai, setAi] = useState<AutoTagProgress | null>(null)
  const qc = useQueryClient()

  useEffect(() => {
    let hideTimer: number | null = null
    const un = onScanProgress((p) => {
      setScan(p)
      if (p.finished) {
        if (hideTimer) window.clearTimeout(hideTimer)
        hideTimer = window.setTimeout(() => setScan(null), 2500)
      }
    })
    return () => {
      un.then((fn) => fn()).catch(() => {})
      if (hideTimer) window.clearTimeout(hideTimer)
    }
  }, [])

  useEffect(() => {
    let hideTimer: number | null = null
    const un = onAutoTagProgress((p) => {
      setAi(p)
      if (p.finished) {
        qc.invalidateQueries({ queryKey: ['images'] })
        qc.invalidateQueries({ queryKey: ['tags'] })
        qc.invalidateQueries({ queryKey: ['image'] })
        if (hideTimer) window.clearTimeout(hideTimer)
        hideTimer = window.setTimeout(() => setAi(null), 2500)
      }
    })
    return () => {
      un.then((fn) => fn()).catch(() => {})
      if (hideTimer) window.clearTimeout(hideTimer)
    }
  }, [qc])

  const bothRunning = scan && !scan.finished && ai && !ai.finished
  const containerClass = bothRunning
    ? 'min-h-[3.25rem] border-t border-surface-border px-3 py-1 flex flex-col justify-center gap-0.5 text-[11px] text-slate-500'
    : 'h-7 border-t border-surface-border px-3 flex items-center text-[11px] text-slate-500 gap-3'

  return (
    <div className={containerClass}>
      {!bothRunning && (
        <span>
          {PRODUCT_NAME} {PRODUCT_VERSION}
        </span>
      )}
      {scan && (
        <ProgressLine
          verb={scan.finished ? 'Scan complete' : 'Scanning'}
          processed={scan.processed}
          total={scan.total}
          finished={scan.finished}
          currentPath={scan.currentPath}
          barClass="bg-accent"
        />
      )}
      {ai && (
        <ProgressLine
          verb={ai.finished ? 'Auto-tag complete' : 'Auto-tagging'}
          processed={ai.processed}
          total={ai.total}
          finished={ai.finished}
          currentPath={ai.currentPath}
          barClass="bg-emerald-500"
          extra={
            ai.tagsAdded > 0 ? (
              <span className="opacity-70">
                {ai.tagsAdded.toLocaleString()} tag
                {ai.tagsAdded === 1 ? '' : 's'} added
              </span>
            ) : null
          }
        />
      )}
      {bothRunning && (
        <span className="opacity-60">
          {PRODUCT_NAME} {PRODUCT_VERSION}
        </span>
      )}
    </div>
  )
}

function ProgressLine({
  verb,
  processed,
  total,
  finished,
  currentPath,
  barClass,
  extra,
}: {
  verb: string
  processed: number
  total: number
  finished: boolean
  currentPath: string | null
  barClass: string
  extra?: React.ReactNode
}) {
  return (
    <div className="flex items-center gap-3">
      <span>·</span>
      <span>
        {verb}
        {!finished && total > 0 && (
          <>
            {' — '}
            {processed.toLocaleString()} / {total.toLocaleString()}
          </>
        )}
      </span>
      {!finished && total > 0 && (
        <div className="flex-1 max-w-xs h-1.5 bg-surface-hover rounded-full overflow-hidden">
          <div
            className={`h-full transition-all ${barClass}`}
            style={{
              width: `${Math.min(100, (processed / Math.max(1, total)) * 100)}%`,
            }}
          />
        </div>
      )}
      {extra}
      {currentPath && !finished && (
        <span className="truncate opacity-60">{currentPath}</span>
      )}
    </div>
  )
}
