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
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary', 'lcov'],
      reportsDirectory: 'coverage',
      include: [
        'src/components/editor/publishWarnings.ts',
        'src/components/nodePreview.ts',
        'src/components/runsFilter.ts',
        'src/components/workflowListFilter.ts',
        'src/errorMessage.ts',
        'src/routing.ts',
      ],
      thresholds: {
        branches: 75,
        functions: 10,
        lines: 80,
        statements: 80,
      },
    },
  },
})
