'use client'

import Link from 'next/link'
import { useMemo, useState } from 'react'
import type { ProjectSummary } from '../../lib/api'

const FAVORITES_KEY = 'argus:favorites:v1'

function relativeTime(value: string): string {
  const delta = Date.now() - new Date(value).getTime()
  const minutes = Math.max(0, Math.floor(delta / 60000))
  if (minutes < 60) return minutes < 1 ? 'just now' : `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days}d ago`
  return new Date(value).toLocaleDateString()
}

function initialFavorites(): string[] {
  if (typeof window === 'undefined') return []
  try { return JSON.parse(window.localStorage.getItem(FAVORITES_KEY) ?? '[]') as string[] } catch { return [] }
}

export default function ProjectList({ projects }: { projects: ProjectSummary[] }) {
  const [query, setQuery] = useState('')
  const [preset, setPreset] = useState('all')
  const [sort, setSort] = useState<'updated' | 'name' | 'tasks'>('updated')
  const [favorites, setFavorites] = useState<string[]>(initialFavorites)

  function toggleFavorite(projectId: string) {
    const key = `project:${projectId}`
    setFavorites((current) => {
      const next = current.includes(key) ? current.filter((item) => item !== key) : [key, ...current]
      window.localStorage.setItem(FAVORITES_KEY, JSON.stringify(next))
      return next
    })
  }

  const visible = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return projects
      .filter((project) => preset === 'all' || project.preset === preset)
      .filter((project) => !normalized || [project.name, project.description, project.preset, project.status, ...project.tags].join(' ').toLowerCase().includes(normalized))
      .sort((left, right) => {
        const leftPinned = favorites.includes(`project:${left.id}`) ? 1 : 0
        const rightPinned = favorites.includes(`project:${right.id}`) ? 1 : 0
        if (leftPinned !== rightPinned) return rightPinned - leftPinned
        if (sort === 'name') return left.name.localeCompare(right.name)
        if (sort === 'tasks') return right.open_tasks - left.open_tasks || Date.parse(right.updated_at) - Date.parse(left.updated_at)
        return Date.parse(right.updated_at) - Date.parse(left.updated_at)
      })
  }, [favorites, preset, projects, query, sort])

  return (
    <section className="panel">
      <div className="panel-header resource-toolbar-header">
        <div><h2>All projects</h2><p>{visible.length} of {projects.length} workspace{projects.length === 1 ? '' : 's'}</p></div>
        <div className="resource-toolbar">
          <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search projects…" aria-label="Search projects" />
          <select value={preset} onChange={(event) => setPreset(event.target.value)} aria-label="Filter project preset">
            <option value="all">All presets</option><option value="empty">Empty</option><option value="software">Software</option><option value="website">Website</option><option value="infrastructure">Infrastructure</option><option value="client">Client</option>
          </select>
          <select value={sort} onChange={(event) => setSort(event.target.value as typeof sort)} aria-label="Sort projects">
            <option value="updated">Recently updated</option><option value="name">Name</option><option value="tasks">Open tasks</option>
          </select>
        </div>
      </div>
      {visible.length === 0 ? (
        <div className="empty-state"><strong>No matching projects</strong>Change the search or filter to show other workspaces.</div>
      ) : (
        <ul className="data-list">
          {visible.map((project) => {
            const pinned = favorites.includes(`project:${project.id}`)
            return (
              <li className="data-row" key={project.id}>
                <button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(project.id)} aria-label={pinned ? `Unpin ${project.name}` : `Pin ${project.name}`} title={pinned ? 'Unpin project' : 'Pin project'}>{pinned ? '★' : '☆'}</button>
                <div>
                  <div className="row-title">
                    <span className="status-dot online" />
                    <Link href={`/projects/${project.id}`}>{project.name}</Link>
                    <span className="badge">{project.preset}</span>
                  </div>
                  <div className="row-subtitle">{project.description || 'No project description'}{project.tags.length ? ` · ${project.tags.join(' · ')}` : ''}</div>
                </div>
                <div className="row-meta">
                  <span>{project.open_tasks} open task{project.open_tasks === 1 ? '' : 's'}</span>
                  <span className="badge success">{project.status}</span>
                  <span title={new Date(project.updated_at).toLocaleString()}>Updated {relativeTime(project.updated_at)}</span>
                </div>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
