import { useRef } from 'react'
import { useStore } from '../store'

/**
 * Search input that displays currently-selected tags as removable
 * chips followed by a free-text FTS input. Chips are bidirectionally
 * synced with the sidebar's tag selection: removing a chip here
 * deselects the tag in the sidebar, and clicking a tag in the
 * sidebar adds a chip here.
 */
export function SearchBox() {
  const selectedTags = useStore((s) => s.selectedTags)
  const removeSelectedTag = useStore((s) => s.removeSelectedTag)
  const search = useStore((s) => s.search)
  const setSearch = useStore((s) => s.setSearch)
  const inputRef = useRef<HTMLInputElement>(null)

  return (
    <div
      className="input flex items-center gap-1 flex-wrap py-1 pl-2 pr-2 min-h-[36px] cursor-text"
      onClick={() => inputRef.current?.focus()}
    >
      {selectedTags.map((t) => (
        <Chip key={t} label={t} onRemove={() => removeSelectedTag(t)} />
      ))}
      <input
        ref={inputRef}
        type="search"
        className="flex-1 bg-transparent border-0 outline-none min-w-[120px] text-sm"
        placeholder={
          selectedTags.length === 0 ? 'Search title, filename, tags…' : 'Add text search…'
        }
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        onKeyDown={(e) => {
          // Backspace on empty text removes the last chip — a common
          // convention in chip-input widgets.
          if (
            e.key === 'Backspace' &&
            search.length === 0 &&
            selectedTags.length > 0
          ) {
            const last = selectedTags[selectedTags.length - 1]!
            removeSelectedTag(last)
          }
        }}
      />
    </div>
  )
}

function Chip({ label, onRemove }: { label: string; onRemove: () => void }) {
  return (
    <span
      className="inline-flex items-center gap-1 rounded bg-accent/25 text-accent-content px-1.5 py-0.5 text-[12px]"
      onClick={(e) => e.stopPropagation()}
    >
      <span>{label}</span>
      <button
        className="opacity-70 hover:opacity-100 leading-none px-0.5"
        title={`Remove tag "${label}"`}
        onClick={(e) => {
          e.stopPropagation()
          onRemove()
        }}
      >
        ×
      </button>
    </span>
  )
}
