import { useEffect, useState } from 'react'
import { onScanProgress } from '../ipc'
import { PRODUCT_NAME, PRODUCT_VERSION } from '../brand'
import type { ScanProgress } from '../types'

export function StatusBar() {
  const [scan, setScan] = useState<ScanProgress | null>(null)

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

  return (
    <div className="h-7 border-t border-surface-border px-3 flex items-center text-[11px] text-slate-500 gap-3">
      <span>{PRODUCT_NAME} {PRODUCT_VERSION}</span>
      {scan && (
        <>
          <span>·</span>
          <span>
            {scan.finished ? 'Scan complete' : 'Scanning'}
            {' — '}
            {scan.processed.toLocaleString()} / {scan.total.toLocaleString()}
          </span>
          {!scan.finished && scan.total > 0 && (
            <div className="flex-1 max-w-xs h-1.5 bg-surface-hover rounded-full overflow-hidden">
              <div
                className="h-full bg-accent transition-all"
                style={{ width: `${Math.min(100, (scan.processed / Math.max(1, scan.total)) * 100)}%` }}
              />
            </div>
          )}
          {scan.currentPath && !scan.finished && (
            <span className="truncate opacity-60">{scan.currentPath}</span>
          )}
        </>
      )}
    </div>
  )
}
