import { useEffect, useRef, useState } from 'react'
import { getThumbPath, toAssetUrl } from '../ipc'
import type { ThumbSize } from '../types'

export function Thumbnail({
  id,
  size = 'small',
  className,
  alt,
}: {
  id: number
  size?: ThumbSize
  className?: string
  alt?: string
}) {
  const [src, setSrc] = useState<string | null>(null)
  const [failed, setFailed] = useState(false)
  const cancelled = useRef(false)

  useEffect(() => {
    cancelled.current = false
    setSrc(null)
    setFailed(false)
    getThumbPath(id, size)
      .then((p) => {
        if (!cancelled.current) setSrc(toAssetUrl(p))
      })
      .catch(() => {
        if (!cancelled.current) setFailed(true)
      })
    return () => {
      cancelled.current = true
    }
  }, [id, size])

  if (failed) {
    return (
      <div
        className={
          'grid place-items-center bg-surface-hover text-slate-500 text-xs ' +
          (className ?? '')
        }
      >
        no preview
      </div>
    )
  }

  if (!src) {
    return (
      <div
        className={
          'bg-surface-hover animate-pulse ' + (className ?? '')
        }
      />
    )
  }

  return (
    <img
      src={src}
      alt={alt ?? ''}
      loading="lazy"
      draggable={false}
      className={className}
      onError={() => setFailed(true)}
    />
  )
}
