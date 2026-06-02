// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import { App } from './App'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
