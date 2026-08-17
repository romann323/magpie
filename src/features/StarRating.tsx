import clsx from 'clsx'

export function StarRating({
  value,
  onChange,
  size = 18,
  interactive = true,
  className,
}: {
  value: number | null
  onChange?: (v: number | null) => void
  size?: number
  interactive?: boolean
  className?: string
}) {
  return (
    <div className={clsx('inline-flex items-center gap-0.5', className)}>
      {[1, 2, 3, 4, 5].map((n) => {
        const active = value != null && value >= n
        return (
          <button
            key={n}
            className={clsx(
              'inline-flex items-center justify-center transition-colors',
              interactive
                ? active
                  ? 'text-star hover:text-yellow-300'
                  : 'text-slate-600 hover:text-star'
                : active
                  ? 'text-star'
                  : 'text-slate-600',
              !interactive && 'pointer-events-none',
            )}
            style={{ width: size + 4, height: size + 4 }}
            title={`${n} star${n === 1 ? '' : 's'}`}
            onClick={(e) => {
              if (!onChange) return
              e.stopPropagation()
              onChange(value === n ? null : n)
            }}
          >
            <svg
              viewBox="0 0 24 24"
              width={size}
              height={size}
              fill={active ? 'currentColor' : 'none'}
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
            </svg>
          </button>
        )
      })}
    </div>
  )
}
