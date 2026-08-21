import { useEffect, useMemo, useRef, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useVirtualizer } from '@tanstack/react-virtual'
import { queryImages } from '../ipc'
import { filterFromView, useStore } from '../store'
import type { ImageSummary } from '../types'
import { Thumbnail } from './Thumbnail'
import clsx from 'clsx'

const CELL_MIN_WIDTH = 200
const CELL_PAD = 12   // tile inner padding on every side
const CELL_GAP = 8    // gap between thumbnail area and label area inside a tile
const CELL_LABELS = 44 // two lines of labels (filename + dimensions/date)
const GRID_GAP = 12   // gap between tiles

export function ImageGrid() {
  const view = useStore((s) => s.view)
  const search = useStore((s) => s.search)
  const sort = useStore((s) => s.sort)
  const extra = useStore((s) => s.extraFilter)
  const selection = useStore((s) => s.selection)
  const select = useStore((s) => s.select)

  const filter = useMemo(() => filterFromView(view, extra, search), [view, extra, search])

  const scrollRef = useRef<HTMLDivElement>(null)
  const [containerWidth, setContainerWidth] = useState(0)

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const ro = new ResizeObserver(() => setContainerWidth(el.clientWidth))
    ro.observe(el)
    setContainerWidth(el.clientWidth)
    return () => ro.disconnect()
  }, [])

  // Simple pagination: request up to 5000 items in one page. Grids with more
  // than that are rare in v1; a follow-up milestone will add proper paging.
  const q = useQuery({
    queryKey: ['images', filter, sort],
    queryFn: () =>
      queryImages({ filter, sort, page: { offset: 0, limit: 5000 } }),
  })

  const items = q.data?.items ?? []

  const cols = Math.max(1, Math.floor((containerWidth + GRID_GAP) / (CELL_MIN_WIDTH + GRID_GAP)))
  const cellWidth = cols > 0 ? (containerWidth - GRID_GAP * (cols - 1)) / cols : CELL_MIN_WIDTH
  // The thumbnail area is a square whose width is (cellWidth − 2×padding); the
  // tile also needs room for padding, the inner gap, and the labels below.
  const thumbSize = Math.max(0, cellWidth - CELL_PAD * 2)
  const rowHeight = Math.round(thumbSize + CELL_PAD * 2 + CELL_GAP + CELL_LABELS)
  const rowCount = Math.ceil(items.length / cols)

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight + GRID_GAP,
    overscan: 4,
  })

  return (
    <main className="min-h-0 h-full flex flex-col">
      <GridStatus total={q.data?.total ?? items.length} loading={q.isFetching} />
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-auto p-3">
        {items.length === 0 && !q.isFetching && (
          <EmptyState />
        )}

        <div
          style={{
            height: virtualizer.getTotalSize(),
            position: 'relative',
            width: '100%',
          }}
        >
          {virtualizer.getVirtualItems().map((v) => {
            const startIdx = v.index * cols
            const endIdx = Math.min(startIdx + cols, items.length)
            return (
              <div
                key={v.key}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  transform: `translateY(${v.start}px)`,
                  width: '100%',
                  height: rowHeight,
                  display: 'grid',
                  gridTemplateColumns: `repeat(${cols}, 1fr)`,
                  gap: GRID_GAP,
                  padding: 0,
                }}
              >
                {items.slice(startIdx, endIdx).map((img) => (
                  <Cell
                    key={img.id}
                    img={img}
                    selected={selection.ids.has(img.id)}
                    onClick={(e) => {
                      const mode: 'add' | 'toggle' | 'replace' = e.shiftKey
                        ? 'add'
                        : e.ctrlKey || e.metaKey
                          ? 'toggle'
                          : 'replace'
                      select(img.id, mode)
                    }}
                  />
                ))}
              </div>
            )
          })}
        </div>
      </div>
    </main>
  )
}

function Cell({
  img,
  selected,
  onClick,
}: {
  img: ImageSummary
  selected: boolean
  onClick: (e: React.MouseEvent) => void
}) {
  // The button is laid out as a CSS grid: a square thumbnail area (the `1fr`
  // row whose track we constrain via aspect-square on the inner div) plus an
  // auto-sized labels area below. This is more predictable than a flex column
  // because the thumbnail's dimensions are driven purely by width, so the
  // image always has a fixed square container to `object-contain` inside.
  return (
    <button
      className={clsx(
        'card text-left transition-shadow grid',
        selected
          ? 'ring-2 ring-accent border-accent'
          : 'hover:border-slate-500'
      )}
      style={{
        padding: CELL_PAD,
        gap: CELL_GAP,
        gridTemplateRows: 'auto auto',
      }}
      onClick={onClick}
    >
      <div className="relative w-full aspect-square bg-surface-hover rounded overflow-hidden">
        <Thumbnail
          id={img.id}
          size="medium"
          className="absolute inset-0 w-full h-full object-contain"
          alt={img.filename}
        />
      </div>
      <div className="min-w-0">
        <div className="text-sm truncate" title={img.title ?? img.filename}>
          {img.title ?? img.filename}
        </div>
        <div className="text-[11px] text-slate-500 truncate">
          {img.width && img.height ? `${img.width}×${img.height}` : ''}{' '}
          {img.takenAt ? '· ' + formatDate(img.takenAt) : ''}
        </div>
      </div>
    </button>
  )
}

function GridStatus({ total, loading }: { total: number; loading: boolean }) {
  return (
    <div className="h-9 shrink-0 border-b border-surface-border flex items-center px-3 text-xs text-slate-500">
      <span>{total.toLocaleString()} images</span>
      {loading && <span className="ml-3 text-slate-400 animate-pulse">loading…</span>}
    </div>
  )
}

function EmptyState() {
  return (
    <div className="h-full grid place-items-center text-slate-500 text-sm">
      <div className="text-center max-w-md">
        <div className="text-5xl mb-3">📷</div>
        <div className="text-slate-300 mb-1">Nothing to show yet</div>
        <div className="text-slate-500">
          Add a folder from the top bar to scan it for images.
        </div>
      </div>
    </div>
  )
}

function formatDate(ms: number) {
  const d = new Date(ms)
  const now = new Date()
  const sameYear = d.getFullYear() === now.getFullYear()
  return d.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: sameYear ? undefined : 'numeric',
  })
}
