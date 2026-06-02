// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { WorkspaceRecord, ProjectRecord } from '../types'
import { useTheme } from '../useTheme'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

export function WorkspacePage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([])
  const [projects, setProjects] = useState<Record<string, ProjectRecord[]>>({})
  const [expanded, setExpanded] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const [addingWs, setAddingWs] = useState(false)
  const [wsName, setWsName] = useState('')
  const [wsDesc, setWsDesc] = useState('')
  const [savingWs, setSavingWs] = useState(false)

  const [addingProj, setAddingProj] = useState<string | null>(null)
  const [projName, setProjName] = useState('')
  const [projDesc, setProjDesc] = useState('')
  const [savingProj, setSavingProj] = useState(false)

  const load = () => {
    setLoading(true)
    setError(null)
    api.listWorkspaces(auth!.tenantId)
      .then(setWorkspaces)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const expandWorkspace = async (wsId: string) => {
    if (expanded === wsId) {
      setExpanded(null)
      return
    }
    setExpanded(wsId)
    if (!projects[wsId]) {
      try {
        const projs = await api.listProjects(auth!.tenantId, wsId)
        setProjects((prev) => ({ ...prev, [wsId]: projs }))
      } catch (e) {
        setError(String(e))
      }
    }
  }

  const handleCreateWorkspace = async () => {
    if (!wsName.trim()) return
    setSavingWs(true)
    try {
      const ws = await api.createWorkspace(auth!.tenantId, wsName.trim(), wsDesc.trim() || undefined)
      setWorkspaces((prev) => [...prev, ws].sort((a, b) => a.name.localeCompare(b.name)))
      setAddingWs(false)
      setWsName('')
      setWsDesc('')
    } catch (e) {
      setError(String(e))
    } finally {
      setSavingWs(false)
    }
  }

  const handleDeleteWorkspace = async (wsId: string) => {
    if (!window.confirm(zh ? '删除此工作空间？' : 'Delete this workspace?')) return
    try {
      await api.deleteWorkspace(auth!.tenantId, wsId)
      setWorkspaces((prev) => prev.filter((w) => w.id !== wsId))
      setProjects((prev) => { const n = { ...prev }; delete n[wsId]; return n })
      if (expanded === wsId) setExpanded(null)
    } catch (e) {
      setError(String(e))
    }
  }

  const handleCreateProject = async () => {
    if (!addingProj || !projName.trim()) return
    setSavingProj(true)
    try {
      const proj = await api.createProject(auth!.tenantId, addingProj, projName.trim(), projDesc.trim() || undefined)
      setProjects((prev) => ({
        ...prev,
        [addingProj]: [...(prev[addingProj] ?? []), proj].sort((a, b) => a.name.localeCompare(b.name)),
      }))
      setAddingProj(null)
      setProjName('')
      setProjDesc('')
    } catch (e) {
      setError(String(e))
    } finally {
      setSavingProj(false)
    }
  }

  const handleDeleteProject = async (wsId: string, projId: string) => {
    if (!window.confirm(zh ? '删除此项目？' : 'Delete this project?')) return
    try {
      await api.deleteProject(auth!.tenantId, projId)
      setProjects((prev) => ({
        ...prev,
        [wsId]: (prev[wsId] ?? []).filter((p) => p.id !== projId),
      }))
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('workspaces.title')}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm btn-primary" onClick={() => setAddingWs(true)}>
            + {zh ? '新建工作空间' : 'New Workspace'}
          </button>
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle dark/light theme'}>
            {theme === 'dark' ? '☀' : '◑'}
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{t('workspaces.title')}</h1>
        </div>

        {error && (
          <div style={{ color: 'var(--danger-text)', marginBottom: 12, fontSize: 13 }}>{error}</div>
        )}

        {loading ? (
          <div style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</div>
        ) : workspaces.length === 0 ? (
          <div className="empty-state">{zh ? '暂无工作空间，创建一个以管理您的项目。' : 'No workspaces yet. Create one to organise your projects.'}</div>
        ) : (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>{zh ? '名称' : 'Name'}</th>
                <th>{zh ? '描述' : 'Description'}</th>
                <th style={{ width: 180 }}></th>
              </tr>
            </thead>
            <tbody>
              {workspaces.map((ws) => (
                <>
                  <tr key={ws.id}>
                    <td>
                      <button
                        style={{ background: 'none', border: 'none', cursor: 'pointer', fontWeight: 600, color: 'var(--fg)', padding: 0, fontSize: 14 }}
                        onClick={() => expandWorkspace(ws.id)}
                      >
                        {expanded === ws.id ? '▾' : '▸'} {ws.name}
                      </button>
                    </td>
                    <td style={{ color: 'var(--muted)', fontSize: 13 }}>{ws.description ?? ''}</td>
                    <td>
                      <div style={{ display: 'flex', gap: 6 }}>
                        <button
                          className="btn btn-sm btn-primary"
                          onClick={() => { setExpanded(ws.id); setAddingProj(ws.id) }}
                        >
                          + {zh ? '项目' : 'Project'}
                        </button>
                        <button
                          className="btn btn-sm btn-danger"
                          onClick={() => handleDeleteWorkspace(ws.id)}
                        >
                          {t('common.delete')}
                        </button>
                      </div>
                    </td>
                  </tr>
                  {expanded === ws.id && (
                    <tr key={ws.id + '-projects'}>
                      <td colSpan={3} style={{ paddingLeft: 28, background: 'var(--surface)' }}>
                        {!projects[ws.id] ? (
                          <div style={{ color: 'var(--muted)', fontSize: 12, padding: '8px 0' }}>{zh ? '加载中…' : 'Loading…'}</div>
                        ) : projects[ws.id].length === 0 ? (
                          <div style={{ color: 'var(--muted)', fontSize: 12, padding: '8px 0' }}>{zh ? '暂无项目。' : 'No projects yet.'}</div>
                        ) : (
                          <table className="workflow-table" style={{ margin: '4px 0' }}>
                            <thead>
                              <tr>
                                <th style={{ fontSize: 12 }}>{zh ? '项目' : 'Project'}</th>
                                <th style={{ fontSize: 12 }}>{zh ? '描述' : 'Description'}</th>
                                <th style={{ width: 80 }}></th>
                              </tr>
                            </thead>
                            <tbody>
                              {projects[ws.id].map((proj) => (
                                <tr key={proj.id}>
                                  <td style={{ fontSize: 13 }}>{proj.name}</td>
                                  <td style={{ fontSize: 12, color: 'var(--muted)' }}>{proj.description ?? ''}</td>
                                  <td>
                                    <button
                                      className="btn btn-sm btn-danger"
                                      onClick={() => handleDeleteProject(ws.id, proj.id)}
                                    >
                                      {t('common.delete')}
                                    </button>
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        )}
                      </td>
                    </tr>
                  )}
                </>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {addingWs && (
        <div className="modal-backdrop" onClick={() => setAddingWs(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? '新建工作空间' : 'New Workspace'}</h2>
            <div className="field">
              <label>{zh ? '名称' : 'Name'}</label>
              <input
                autoFocus
                placeholder={zh ? '如 工程团队' : 'e.g. Engineering'}
                value={wsName}
                onChange={(e) => setWsName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleCreateWorkspace() }}
              />
            </div>
            <div className="field">
              <label>{zh ? '描述（可选）' : 'Description (optional)'}</label>
              <input
                placeholder={zh ? '简短描述' : 'Short description'}
                value={wsDesc}
                onChange={(e) => setWsDesc(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleCreateWorkspace() }}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => { setAddingWs(false); setWsName(''); setWsDesc('') }}>
                {t('common.cancel')}
              </button>
              <button
                className="btn btn-primary"
                disabled={savingWs || !wsName.trim()}
                onClick={handleCreateWorkspace}
              >
                {savingWs ? (zh ? '创建中…' : 'Creating…') : t('common.create')}
              </button>
            </div>
          </div>
        </div>
      )}

      {addingProj !== null && (
        <div className="modal-backdrop" onClick={() => setAddingProj(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? '新建项目' : 'New Project'}</h2>
            <div className="field">
              <label>{zh ? '名称' : 'Name'}</label>
              <input
                autoFocus
                placeholder={zh ? '如 后端' : 'e.g. Backend'}
                value={projName}
                onChange={(e) => setProjName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleCreateProject() }}
              />
            </div>
            <div className="field">
              <label>{zh ? '描述（可选）' : 'Description (optional)'}</label>
              <input
                placeholder={zh ? '简短描述' : 'Short description'}
                value={projDesc}
                onChange={(e) => setProjDesc(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleCreateProject() }}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => { setAddingProj(null); setProjName(''); setProjDesc('') }}>
                {t('common.cancel')}
              </button>
              <button
                className="btn btn-primary"
                disabled={savingProj || !projName.trim()}
                onClick={handleCreateProject}
              >
                {savingProj ? (zh ? '创建中…' : 'Creating…') : t('common.create')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
