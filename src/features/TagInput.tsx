import { useEffect, useMemo, useRef, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { listTags } from '../ipc'

export function TagInput({
  tags,
  onChange,
}: {
  tags: string[]
  onChange: (next: string[]) => void
}) {
  const [draft, setDraft] = useState('')
  const [showSuggest, setShowSuggest] = useState(false)
  const wrapRef = useRef<HTMLDivElement>(null)

  const allTags = useQuery({
    queryKey: ['tags', 'all'],
    queryFn: () => listTags(),
    staleTime: 30_000,
  })

  const suggestions = useMemo(() => {
    const q = draft.trim().toLowerCase()
    if (!q) return []
    const owned = new Set(tags.map((t) => t.toLowerCase()))
    return (allTags.data ?? [])
      .filter((t) => t.name.toLowerCase().includes(q) && !owned.has(t.name.toLowerCase()))
      .slice(0, 8)
  }, [draft, allTags.data, tags])

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!wrapRef.current) return
      if (!wrapRef.current.contains(e.target as Node)) setShowSuggest(false)
    }
    window.addEventListener('mousedown', onDown)
    return () => window.removeEventListener('mousedown', onDown)
  }, [])

  function addTag(t: string) {
    const clean = t.trim()
    if (!clean) return
    if (tags.some((x) => x.toLowerCase() === clean.toLowerCase())) return
    onChange([...tags, clean])
    setDraft('')
    setShowSuggest(false)
  }

  function removeTag(t: string) {
    onChange(tags.filter((x) => x !== t))
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    // Commit the current draft as a tag on Enter, comma, or Space.
    // (Space is convenient when a tag is a single word; use "," or Enter for
    // multi-word tags entered as separate items.)
    if (e.key === 'Enter' || e.key === ',' || e.key === ' ') {
      if (draft.trim()) {
        e.preventDefault()
        addTag(draft)
      } else if (e.key === 'Enter') {
        // Prevent stray form submits when the draft is empty.
        e.preventDefault()
      }
    } else if (e.key === 'Backspace' && draft === '' && tags.length > 0) {
      removeTag(tags[tags.length - 1]!)
    }
  }

  return (
    <div className="relative" ref={wrapRef}>
      <div
        className="min-h-[38px] w-full px-2 py-1 rounded-md bg-surface-raised border border-surface-border flex flex-wrap items-center gap-1 focus-within:border-accent focus-within:ring-1 focus-within:ring-accent"
        onClick={() => wrapRef.current?.querySelector('input')?.focus()}
      >
        {tags.map((t) => (
          <span
            key={t}
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-accent/15 border border-accent/30 text-accent text-xs"
          >
            {t}
            <button
              className="text-accent/70 hover:text-white"
              onClick={(e) => {
                e.stopPropagation()
                removeTag(t)
              }}
              title="Remove tag"
            >
              ×
            </button>
          </span>
        ))}
        <input
          className="flex-1 min-w-[80px] bg-transparent outline-none text-sm text-slate-200 placeholder-slate-500"
          placeholder={tags.length === 0 ? 'Add tags (Enter or Space to add)' : ''}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value)
            setShowSuggest(true)
          }}
          onFocus={() => setShowSuggest(true)}
          onKeyDown={onKeyDown}
          onBlur={() => {
            // If the user typed a tag and clicked away without pressing Enter,
            // still commit it — otherwise their typing is silently discarded.
            if (draft.trim()) addTag(draft)
          }}
        />
      </div>

      {showSuggest && suggestions.length > 0 && (
        <div className="absolute z-10 left-0 right-0 mt-1 card overflow-hidden">
          {suggestions.map((s) => (
            <button
              key={s.name}
              className="w-full text-left px-3 py-1.5 text-sm hover:bg-surface-hover flex items-center justify-between"
              onClick={() => addTag(s.name)}
            >
              <span>{s.name}</span>
              <span className="text-xs text-slate-500">{s.count}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
