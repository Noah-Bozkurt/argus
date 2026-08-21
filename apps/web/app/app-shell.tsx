'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import type { ReactNode } from 'react'
import type { SessionUser } from '../lib/auth'
import CommandPalette from './command-palette'
import NotificationIndicator from './notification-indicator'

type IconName = 'overview' | 'projects' | 'servers' | 'jobs' | 'notifications' | 'settings' | 'logout'

function Icon({ name }: { name: IconName }) {
  const paths: Record<IconName, ReactNode> = {
    overview: <><rect x="3" y="3" width="7" height="7" rx="2"/><rect x="14" y="3" width="7" height="7" rx="2"/><rect x="3" y="14" width="7" height="7" rx="2"/><rect x="14" y="14" width="7" height="7" rx="2"/></>,
    projects: <><path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H9l2 2h7.5A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z"/></>,
    servers: <><rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01M7 17h.01M11 7h6M11 17h6"/></>,
    jobs: <><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></>,
    notifications: <><path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9"/><path d="M10 21h4"/></>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.1A1.7 1.7 0 0 0 8 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 3.6 15a1.7 1.7 0 0 0-1.5-1H2v-4h.1A1.7 1.7 0 0 0 3.6 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 8 4.6a1.7 1.7 0 0 0 1-1.5V3h4v.1A1.7 1.7 0 0 0 14 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.5 1h.1v4h-.1a1.7 1.7 0 0 0-1.5 1z"/></>,
    logout: <><path d="M10 17l5-5-5-5"/><path d="M15 12H3"/><path d="M14 3h5a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-5"/></>,
  }

  return <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>
}

const nav = [
  { label: 'Overview', href: '/', icon: 'overview' as const },
  { label: 'Projects', href: '/projects', icon: 'projects' as const },
  { group: 'Infrastructure' },
  { label: 'Servers', href: '/infrastructure/servers', icon: 'servers' as const },
  { group: 'Operations' },
  { label: 'Jobs', href: '/jobs', icon: 'jobs' as const },
  { label: 'Notifications', href: '/notifications', icon: 'notifications' as const },
  { label: 'System', href: '/system', icon: 'settings' as const },
]

function titleFromPath(pathname: string): string {
  if (pathname === '/') return 'Overview'
  const parts = pathname.split('/').filter(Boolean)
  if (parts[0] === 'projects' && parts.length > 1) return 'Project workspace'
  if (parts[0] === 'infrastructure' && parts[1] === 'servers' && parts.length > 2) return 'Server details'
  return parts.at(-1)?.replaceAll('-', ' ').replace(/^./, (value) => value.toUpperCase()) ?? 'Argus'
}

function userInitial(user: SessionUser | null): string {
  const source = user?.displayName?.trim() || user?.email || 'A'
  return source.charAt(0).toUpperCase()
}

export default function AppShell({ children, user }: { children: ReactNode; user: SessionUser | null }) {
  const pathname = usePathname()
  const publicPage = pathname.startsWith('/status/') || pathname === '/healthz' || pathname === '/login'

  if (publicPage) return <>{children}</>

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <Link className="brand" href="/" aria-label="Argus overview">
          <span className="brand-mark">A</span>
          <span>
            <strong>ARGUS</strong>
            <small>Control plane</small>
          </span>
        </Link>

        <nav className="sidebar-nav" aria-label="Primary navigation">
          {nav.map((item, index) => {
            if ('group' in item) return <div className="nav-group" key={`${item.group}-${index}`}>{item.group}</div>
            const active = item.href === '/' ? pathname === '/' : pathname.startsWith(item.href)
            return (
              <Link className={`nav-link${active ? ' active' : ''}`} href={item.href} key={item.href} aria-current={active ? 'page' : undefined}>
                <Icon name={item.icon} />
                <span>{item.label}</span>
              </Link>
            )
          })}
        </nav>

        <div className="sidebar-footer">
          {user ? (
            <div className="signed-in-user">
              <strong>{user.displayName || user.email}</strong>
              <small>{user.role}</small>
            </div>
          ) : null}
          <div className="nav-link muted"><Icon name="settings" /><span>Settings</span></div>
          <form action="/auth/logout" method="post">
            <button className="nav-link logout-button" type="submit"><Icon name="logout" /><span>Sign out</span></button>
          </form>
          <div className="version">Argus <span>v0.1.0</span></div>
        </div>
      </aside>

      <div className="workspace-shell">
        <header className="topbar">
          <div className="topbar-title">
            <span className="eyebrow">Workspace</span>
            <strong>{titleFromPath(pathname)}</strong>
          </div>
          <div className="topbar-actions">
            <CommandPalette />
            <span className="control-status"><span className="status-dot online" />Control plane</span>
            <Link className="icon-button notification-button" href="/notifications" aria-label="Notifications"><Icon name="notifications" /><NotificationIndicator /></Link>
            <form className="mobile-signout" action="/auth/logout" method="post">
              <button className="icon-button" type="submit" aria-label="Sign out" title="Sign out"><Icon name="logout" /></button>
            </form>
            <div className="avatar" title={user?.email ?? 'Argus account'}>{userInitial(user)}</div>
          </div>
        </header>
        <div className="page-frame">{children}</div>
      </div>
    </div>
  )
}
