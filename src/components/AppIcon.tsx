/** Medallion app icon (option 5) — shared by TopBar badge and inline UI. */
export function AppIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 128 128"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect width="128" height="128" rx="28" fill="#1E1B4B" />
      <circle cx="64" cy="64" r="44" fill="#4F46E5" />
      <circle cx="64" cy="64" r="36" fill="#312E81" />
      <path
        fill="#F8FAFC"
        d="M64 34c14 0 26 10 30 24 2 6 2 12 0 18-4 14-16 24-30 24s-26-10-30-24c-2-6-2-12 0-18 4-14 16-24 30-24zm0 8c-10 0-18 8-20 18-1 4-1 8 0 12 2 10 10 18 20 18s18-8 20-18c1-4 1-8 0-12-2-10-10-18-20-18z"
      />
      <path
        fill="#0F172A"
        d="M64 42c8 0 14 6 16 14 1 3 1 6 0 9-2 8-8 14-16 14s-14-6-16-14c-1-3-1-6 0-9 2-8 8-14 16-14z"
      />
      <circle cx="70" cy="50" r="2.5" fill="#F8FAFC" />
      <path fill="#A5B4FC" d="M78 56l8 4-8 3 2-7z" />
    </svg>
  )
}
