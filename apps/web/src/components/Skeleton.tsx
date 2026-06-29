// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Lightweight loading placeholders. Replaces the bare "Loading…" text on data
// pages with shimmering rows so the layout doesn't lurch when content arrives.

export function SkeletonRows({ rows = 6, gap = 10 }: { rows?: number; gap?: number }) {
  return (
    <div data-testid="skeleton" aria-busy="true" aria-live="polite" style={{ display: 'flex', flexDirection: 'column', gap, padding: '8px 0' }}>
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="skeleton skeleton-row" style={{ width: `${88 - (i % 3) * 12}%` }} />
      ))}
    </div>
  )
}

export function Skeleton({ width = '100%', height = 14, radius = 6 }: { width?: number | string; height?: number; radius?: number }) {
  return <div className="skeleton" style={{ width, height, borderRadius: radius }} />
}
