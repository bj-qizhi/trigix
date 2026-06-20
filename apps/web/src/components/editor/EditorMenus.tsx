// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { PiCheckCircle, PiCalendarBlank, PiListBullets, PiTestTube, PiChatCircle, PiBookOpen } from 'react-icons/pi'
import type { TranslationKey } from '../../i18n'

export type BgVariant = 'dots' | 'grid' | 'lines' | 'none'

// Toolbar "View" dropdown: snap-to-grid / minimap toggles and the canvas
// background style. Owns its own open/close state. Extracted verbatim from
// WorkflowEditor's toolbar.
export interface ViewMenuProps {
  snapToGrid: boolean
  setSnapToGrid: Dispatch<SetStateAction<boolean>>
  showMinimap: boolean
  setShowMinimap: Dispatch<SetStateAction<boolean>>
  bgVariant: BgVariant
  setBgVariant: Dispatch<SetStateAction<BgVariant>>
  zh: boolean
}

export function ViewMenu({
  snapToGrid, setSnapToGrid, showMinimap, setShowMinimap, bgVariant, setBgVariant, zh,
}: ViewMenuProps) {
  const [open, setOpen] = useState(false)
  return (
    <span className="tb-pop-wrap">
      <button className="btn btn-sm" onClick={() => setOpen((v) => !v)} title={zh ? '画布视图选项' : 'Canvas view options'}>{zh ? '视图' : 'View'} ▾</button>
      {open && (
        <div className="tb-popover tb-menu" onMouseLeave={() => setOpen(false)}>
          <button className="tb-menu-item" onClick={() => setSnapToGrid((v) => !v)}>
            <span>{zh ? '对齐网格' : 'Snap to grid'}</span><span className={`tb-menu-state${snapToGrid ? ' on' : ''}`}>{snapToGrid ? 'ON' : 'OFF'}</span>
          </button>
          <button className="tb-menu-item" onClick={() => setShowMinimap((v) => !v)}>
            <span>{zh ? '小地图' : 'Minimap'}</span><span className={`tb-menu-state${showMinimap ? ' on' : ''}`}>{showMinimap ? 'ON' : 'OFF'}</span>
          </button>
          <div className="tb-menu-item" style={{ cursor: 'default' }}>
            <span>{zh ? '背景' : 'Background'}</span>
            <select
              value={bgVariant}
              onChange={(e) => { const v = e.target.value as BgVariant; setBgVariant(v); try { localStorage.setItem('af:canvas_bg', v) } catch { /* ignore */ } }}
              style={{ fontSize: 11, padding: '2px 6px', borderRadius: 'var(--radius-sm)', border: '1px solid var(--border)', background: 'var(--panel)', color: 'var(--text)', cursor: 'pointer' }}
            >
              <option value="dots">{zh ? '· 点' : '· Dots'}</option>
              <option value="grid">{zh ? '⊹ 网格' : '⊹ Grid'}</option>
              <option value="lines">{zh ? '— 线' : '— Lines'}</option>
              <option value="none">{zh ? '□ 无' : '□ None'}</option>
            </select>
          </div>
        </div>
      )}
    </span>
  )
}

// Toolbar "More" dropdown: validate / schedule / forms / tests / comments /
// API docs. Owns its own open/close state; each item closes the menu then fires
// the corresponding callback. Extracted verbatim from WorkflowEditor's toolbar.
export interface MoreActionsMenuProps {
  zh: boolean
  t: (key: TranslationKey) => string
  onValidate: () => void
  onSchedule: () => void
  onForms: () => void
  onTests: () => void
  onComments: () => void
  onApiDocs: () => void
}

export function MoreActionsMenu({
  zh, t, onValidate, onSchedule, onForms, onTests, onComments, onApiDocs,
}: MoreActionsMenuProps) {
  const [open, setOpen] = useState(false)
  const run = (fn: () => void) => { setOpen(false); fn() }
  return (
    <span className="tb-pop-wrap">
      <button className="btn btn-sm" onClick={() => setOpen((v) => !v)} title={zh ? '更多操作' : 'More actions'}>⋯ {zh ? '更多' : 'More'}</button>
      {open && (
        <div className="tb-popover tb-menu" style={{ right: 0, left: 'auto' }} onMouseLeave={() => setOpen(false)}>
          <button className="tb-menu-item" onClick={() => run(onValidate)}><span className="tb-menu-ic"><PiCheckCircle size={14} />{t('we.validate')}</span></button>
          <button className="tb-menu-item" onClick={() => run(onSchedule)}><span className="tb-menu-ic"><PiCalendarBlank size={14} />{t('we.schedule')}</span></button>
          <button className="tb-menu-item" onClick={() => run(onForms)}><span className="tb-menu-ic"><PiListBullets size={14} />{t('we.form')}</span></button>
          <button className="tb-menu-item" onClick={() => run(onTests)}><span className="tb-menu-ic"><PiTestTube size={14} />{t('we.tests')}</span></button>
          <button className="tb-menu-item" onClick={() => run(onComments)}><span className="tb-menu-ic"><PiChatCircle size={14} />{t('we.comments')}</span></button>
          <button className="tb-menu-item" onClick={() => run(onApiDocs)}><span className="tb-menu-ic"><PiBookOpen size={14} />{zh ? 'API 文档' : 'API Docs'}</span></button>
        </div>
      )}
    </span>
  )
}
