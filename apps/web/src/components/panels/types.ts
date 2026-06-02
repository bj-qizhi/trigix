// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

export interface ConfigProps {
  config: Record<string, unknown>
  set: (key: string, value: unknown) => void
  str: (key: string, fallback?: string) => string
  num: (key: string, fallback: number) => number
}
