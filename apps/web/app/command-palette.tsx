'use client'

import { useEffect, useMemo, useRef, useState } from 'react'
import { usePathname, useRouter } from 'next/navigation'

type PaletteItem = {
  id: string
  kind: 'project' | 'server' | 'job'
  label: string
  description: string
  href: string
  status?: string
  keywords: string[]
}

type PaletteResponse = {
  items: PaletteItem[]
  partial: { projects: boolean; servers: boolean; jobs: boolean }
}

type RecentItem = Pick<PaletteItem, 'id' | 'kind' | 'label' | 'description' | 'href' | 'status'> & { visitedAt: number }

type Command = {
  id: string
  label: string
  description: string
  href?: string
  keywords?: string[]
  action?: 'copy-url' | 'reload' | 'notifications'
}

const FAVORITES_KEY = 'argus:favorites:v1'
const RECENTS_KEY = 'argus:recents:v1'

const navigationCommands: Command[] = [
  { id: 'nav:overview', label: 'Overview', description: 'Control-plane overview', href: '/', keywords: ['home', 'dashboard'] },
  { id: 'nav:projects', label: 'Projects', description: 'Project workspaces', href: '/projects', keywords: ['repositories', 'work'] },
  { id: 'nav:servers', label: 'Servers', description: 'Managed infrastructure', href: '/infrastructure/servers', keywords: ['hosts', 'nodes', 'infrastructure'] },
  { id: 'nav:jobs', label: 'Jobs', description: 'Background and scheduled work', href: '/jobs', keywords: ['queue', 'scheduled'] },
  { id: 'nav:notifications', label: 'Notifications', description: 'Operational notifications', href: '/notifications', keywords: ['alerts', 'inbox'] },
  { id: 'nav:system', label: 'System', description: 'Argus system and updates', href: '/system', keywords: ['update', 'version'] },
]

const utilityCommands: Command[] = [
  { id: 'action:copy-url', label: 'Copy current URL', description: 'Copy this Argus page to the clipboard', action: 'copy-url', keywords: ['link', 'clipboard'] },
  { id: 'action:reload', label: 'Reload current page', description: 'Refresh live data for this view', action: 'reload', keywords: ['refresh'] },
  { id: 'action:notifications', label: 'Enable browser notifications', description: 'Ask this browser for notification permission', action: 'notifications', keywords: ['push', 'permission'] },
]

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key)
    return raw ? JSON.parse(raw) as T : fallback
  } catch {
    return fallback
  }
}

function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null
  if (!element) return false
  return element.tagName === 'INPUT' || element.tagName === 'TEXTAREA' || element.tagName === 'SELECT' || element.isContentEditable
}

function searchableText(item: PaletteItem | Command): string {
  const keywords = 'keywords' in item ? item.keywords ?? [] : []
  return [item.label, item.description, ...keywords, 'status' in item ? item.status ?? '' : ''].join(' ').toLowerCase()
}

function score(text: string, query: string): number {
  if (!query) return 1
  const normalized = query.trim().toLowerCase()
  if (!normalized) return 1
  if (text.startsWith(normalized)) return 100
  const words = normalized.split(/\s+/).filter(Boolean)
  if (words.every((word) => text.includes(word))) return 50 + words.length
  return 0
}

function kindLabel(kind: PaletteItem['kind']): string {
  if (kind === 'project') return 'Project'
  if (kind === 'server') return 'Server'
  return 'Job'
}

export default function CommandPalette() {
  const pathname = usePathname()
  const router = useRouter()
  const inputRef = useRef<HTMLInputElement>(null)
  const sequenceRef = useRef<{ key: string; at: number } | null>(null)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [items, setItems] = useState<PaletteItem[]>([])
  const [partial, setPartial] = useState<PaletteResponse['partial'] | null>(null)
  const [loading, setLoading] = useState(false)
  const [selected, setSelected] = useState(0)
  const [favorites, setFavorites] = useState<string[]>([])
  const [recents, setRecents] = useState<RecentItem[]>([])

  async function loadItems() {
    setLoading(true)
    try {
      const response = await fetch('/api/command-palette', { cache: 'no-store' })
      if (!response.ok) throw new Error(`Palette search failed with ${response.status}`)
      const data = await response.json() as PaletteResponse
      setItems(data.items)
      setPartial(data.partial)

      const current = data.items.find((item) => item.href === pathname)
      if (current) {
        const next: RecentItem[] = [
          { ...current, visitedAt: Date.now() },
          ...readJson<RecentItem[]>(RECENTS_KEY, []).filter((item) => item.id !== current.id),
        ].slice(0, 10)
        window.localStorage.setItem(RECENTS_KEY, JSON.stringify(next))
        setRecents(next)
      }
    } catch (error) {
      console.error(error)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    setFavorites(readJson<string[]>(FAVORITES_KEY, []))
    setRecents(readJson<RecentItem[]>(RECENTS_KEY, []))
    void loadItems()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathname])

  useEffect(() => {
    if (!open) return
    setQuery('')
    setSelected(0)
    window.setTimeout(() => inputRef.current?.focus(), 0)
    void loadItems()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setOpen((value) => !value)
        return
      }
      if (event.key === '/' && !open && !isTypingTarget(event.target)) {
        event.preventDefault()
        setOpen(true)
        return
      }
      if (event.key === 'Escape' && open) {
        event.preventDefault()
        setOpen(false)
        return
      }
      if (open || event.metaKey || event.ctrlKey || event.altKey || isTypingTarget(event.target)) return

      const now = Date.now()
      const previous = sequenceRef.current
      if (previous?.key === 'g' && now - previous.at < 800) {
        const destinations: Record<string, string> = { o: '/', p: '/projects', s: '/infrastructure/servers', j: '/jobs', n: '/notifications' }
        const destination = destinations[event.key.toLowerCase()]
        sequenceRef.current = null
        if (destination) {
          event.preventDefault()
          router.push(destination)
          return
        }
      }
      sequenceRef.current = event.key.toLowerCase() === 'g' ? { key: 'g', at: now } : null
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [open, router])

  const dynamicById = useMemo(() => new Map(items.map((item) => [item.id, item])), [items])
  const currentItem = items.find((item) => item.href === pathname)

  const results = useMemo(() => {
    const q = query.trim().toLowerCase()
    const favoriteItems = favorites.map((id) => dynamicById.get(id)).filter((item): item is PaletteItem => Boolean(item))
    const recentItems = recents
      .map((recent) => dynamicById.get(recent.id) ?? ({ ...recent, keywords: [] } as PaletteItem))
      .filter((item) => !favorites.includes(item.id))

    const dynamicResults = items
      .map((item) => ({ item, score: score(searchableText(item), q) }))
      .filter(({ score: value }) => value > 0)
      .sort((a, b) => b.score - a.score || a.item.label.localeCompare(b.item.label))
      .map(({ item }) => item)
      .filter((item) => !favorites.includes(item.id) && !recentItems.some((recent) => recent.id === item.id))

    const commands = [...navigationCommands, ...utilityCommands]
      .map((item) => ({ item, score: score(searchableText(item), q) }))
      .filter(({ score: value }) => value > 0)
      .sort((a, b) => b.score - a.score)
      .map(({ item }) => item)

    if (q) {
      return [
        ...favoriteItems.filter((item) => score(searchableText(item), q) > 0),
        ...recentItems.filter((item) => score(searchableText(item), q) > 0),
        ...dynamicResults,
        ...commands,
      ].slice(0, 40)
    }

    return [...favoriteItems, ...recentItems.slice(0, 5), ...commands, ...dynamicResults.slice(0, 12)].slice(0, 40)
  }, [dynamicById, favorites, items, query, recents])

  useEffect(() => {
    setSelected((value) => Math.min(value, Math.max(0, results.length - 1)))
  }, [results.length])

  function toggleFavorite(id: string) {
    setFavorites((current) => {
      const next = current.includes(id) ? current.filter((value) => value !== id) : [id, ...current]
      window.localStorage.setItem(FAVORITES_KEY, JSON.stringify(next))
      return next
    })
  }

  async function run(result: PaletteItem | Command) {
    if ('href' in result && result.href) {
      setOpen(false)
      router.push(result.href)
      return
    }
    if (!('action' in result)) return
    if (result.action === 'copy-url') {
      await navigator.clipboard.writeText(window.location.href)
      setOpen(false)
      return
    }
    if (result.action === 'reload') {
      setOpen(false)
      router.refresh()
      return
    }
    if (result.action === 'notifications') {
      if ('Notification' in window) await Notification.requestPermission()
      setOpen(false)
    }
  }

  function onPaletteKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setSelected((value) => Math.min(results.length - 1, value + 1))
    } else if (event.key === 'ArrowUp') {
      event.preventDefault()
      setSelected((value) => Math.max(0, value - 1))
    } else if (event.key === 'Enter' && results[selected]) {
      event.preventDefault()
      void run(results[selected])
    }
  }

  return (
    <>
      <button className="command-trigger" type="button" onClick={() => setOpen(true)} aria-label="Open command palette">
        <span className="command-trigger-search">Search or jump to…</span>
        <kbd>⌘K</kbd>
      </button>
      {open ? (
        <div className="command-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setOpen(false) }}>
          <div className="command-dialog" role="dialog" aria-modal="true" aria-label="Argus command palette">
            <div className="command-input-row">
              <span aria-hidden="true">⌕</span>
              <input ref={inputRef} value={query} onChange={(event) => { setQuery(event.target.value); setSelected(0) }} onKeyDown={onPaletteKeyDown} placeholder="Projects, servers, jobs, pages or commands…" aria-label="Search Argus" />
              <kbd>Esc</kbd>
            </div>
            <div className="command-meta">
              <span>{loading ? 'Refreshing index…' : `${items.length} resources indexed`}</span>
              {currentItem ? <button type="button" className="command-inline-action" onClick={() => toggleFavorite(currentItem.id)}>{favorites.includes(currentItem.id) ? '★ Unpin current' : '☆ Pin current'}</button> : null}
            </div>
            <div className="command-results" role="listbox" aria-label="Search results">
              {!results.length ? <div className="command-empty"><strong>No matches</strong><span>Try a project, hostname, job kind, status or navigation target.</span></div> : results.map((result, index) => {
                const dynamic = 'kind' in result
                const isFavorite = dynamic && favorites.includes(result.id)
                return (
                  <div className={`command-result${selected === index ? ' selected' : ''}`} key={result.id} role="option" aria-selected={selected === index} onMouseEnter={() => setSelected(index)}>
                    <button className="command-result-main" type="button" onClick={() => void run(result)}>
                      <span className="command-kind">{dynamic ? kindLabel(result.kind) : result.id.startsWith('nav:') ? 'Go to' : 'Action'}</span>
                      <span className="command-result-copy"><strong>{result.label}</strong><small>{result.description}</small></span>
                      {dynamic && result.status ? <span className={`badge ${result.status === 'ONLINE' || result.status === 'ACTIVE' || result.status === 'SUCCEEDED' ? 'success' : result.status === 'OFFLINE' || result.status === 'DEAD' ? 'danger' : ''}`}>{result.status}</span> : null}
                    </button>
                    {dynamic && result.kind !== 'job' ? <button type="button" className="command-favorite" aria-label={isFavorite ? `Unpin ${result.label}` : `Pin ${result.label}`} onClick={() => toggleFavorite(result.id)}>{isFavorite ? '★' : '☆'}</button> : null}
                  </div>
                )
              })}
            </div>
            <div className="command-footer">
              <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span><span><kbd>↵</kbd> open</span><span><kbd>g</kbd> then <kbd>p</kbd>/<kbd>s</kbd>/<kbd>j</kbd> jump</span>
              {partial && Object.values(partial).some(Boolean) ? <span className="command-partial">Some resource groups are temporarily unavailable</span> : null}
            </div>
          </div>
        </div>
      ) : null}
    </>
  )
}
