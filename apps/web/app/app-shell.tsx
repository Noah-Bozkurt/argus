'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import type { ReactNode } from 'react'
import type { SessionUser } from '../lib/auth'
import CommandPalette from './command-palette'
import LucideIcon, { type LucideIconName } from './lucide-icons'
import NotificationIndicator from './notification-indicator'

type NavItem = { label: string; href: string; icon: LucideIconName }

const workspaceNav: NavItem[] = [
  { label: 'Overview', href: '/', icon: 'overview' },
  { label: 'Projects', href: '/projects', icon: 'projects' },
]

const operationsNav: NavItem[] = [
  { label: 'Servers', href: '/infrastructure/servers', icon: 'servers' },
  { label: 'Jobs', href: '/jobs', icon: 'jobs' },
  { label: 'Notifications', href: '/notifications', icon: 'notifications' },
  { label: 'System', href: '/system', icon: 'system' },
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

function NavLink({ item, pathname }: { item: NavItem; pathname: string }) {
  const active = item.href === '/' ? pathname === '/' : pathname.startsWith(item.href)
  return (
    <Link className={`nav-link${active ? ' active' : ''}`} href={item.href} aria-current={active ? 'page' : undefined}>
      <LucideIcon name={item.icon} />
      <span>{item.label}</span>
    </Link>
  )
}

export default function AppShell({ children, user }: { children: ReactNode; user: SessionUser | null }) {
  const pathname = usePathname()
  const publicPage = pathname.startsWith('/status/') || pathname === '/healthz' || pathname === '/login'

  if (publicPage) return <>{children}</>

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <Link className="brand" href="/" aria-label="Argus overview">
          <span className="brand-mark"><img src="/argus-icon.svg" alt="" /></span>
          <span className="brand-copy">
            <strong>Argus</strong>
            <small>Control plane</small>
          </span>
        </Link>

        <nav className="sidebar-nav" aria-label="Primary navigation">
          <div className="nav-section">
            <div className="nav-group">Workspace</div>
            {workspaceNav.map((item) => <NavLink key={item.href} item={item} pathname={pathname} />)}
          </div>
          <div className="nav-section">
            <div className="nav-group">Manage</div>
            {operationsNav.map((item) => <NavLink key={item.href} item={item} pathname={pathname} />)}
          </div>
        </nav>

        <div className="sidebar-footer">
          {user ? (
            <div className="account-summary">
              <div className="avatar" aria-hidden="true">{userInitial(user)}</div>
              <div className="signed-in-user">
                <strong>{user.displayName || user.email}</strong>
                <small>{user.role}</small>
              </div>
            </div>
          ) : null}
          <form action="/auth/logout" method="post">
            <button className="nav-link logout-button" type="submit"><LucideIcon name="logout" /><span>Sign out</span></button>
          </form>
          <div className="version">Argus <span>v0.1.0</span></div>
        </div>
      </aside>

      <div className="workspace-shell">
        <header className="topbar">
          <div className="topbar-title">
            <strong>{titleFromPath(pathname)}</strong>
          </div>
          <div className="topbar-actions">
            <CommandPalette />
            <span className="control-status"><span className="status-dot online" />Control plane online</span>
            <Link className="icon-button notification-button" href="/notifications" aria-label="Notifications"><LucideIcon name="notifications" /><NotificationIndicator /></Link>
            <form className="mobile-signout" action="/auth/logout" method="post">
              <button className="icon-button" type="submit" aria-label="Sign out" title="Sign out"><LucideIcon name="logout" /></button>
            </form>
            <div className="topbar-avatar avatar" title={user?.email ?? 'Argus account'}>{userInitial(user)}</div>
          </div>
        </header>
        <div className="page-frame">{children}</div>
      </div>
    </div>
  )
}
