// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Dev seed IDs — used when Platform runs in in-memory mode (default).
// Switch to PostgreSQL UUIDs by setting VITE_TENANT_ID etc. via .env.local.
export const TENANT_ID = import.meta.env.VITE_TENANT_ID ?? 'tenant-1'
export const WORKSPACE_ID = import.meta.env.VITE_WORKSPACE_ID ?? 'workspace-1'
export const PROJECT_ID = import.meta.env.VITE_PROJECT_ID ?? 'project-1'
