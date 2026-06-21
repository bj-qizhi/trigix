// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { defineConfig } from 'vitest/config'

// Unit tests cover the pure helper functions only (src/**/*.test.ts). The
// Playwright end-to-end specs under e2e/ are *.spec.ts and run via `npm run e2e`,
// so they are deliberately not matched here.
export default defineConfig({
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
})
