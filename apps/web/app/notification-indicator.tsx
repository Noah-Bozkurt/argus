'use client'

import { useEffect, useRef, useState } from 'react'
import type { NotificationInbox } from '../lib/notifications-api'

const SEEN_KEY = 'argus:browser-notifications:v1'

function readSeen(): string[] {
  try { return JSON.parse(window.localStorage.getItem(SEEN_KEY) ?? '[]') as string[] } catch { return [] }
}

export default function NotificationIndicator() {
  const [count, setCount] = useState(0)
  const firstLoad = useRef(true)

  useEffect(() => {
    let cancelled = false

    async function refresh() {
      if (document.visibilityState === 'hidden') return
      try {
        const response = await fetch('/api/notifications/inbox', { cache: 'no-store' })
        if (!response.ok) return
        const inbox = await response.json() as NotificationInbox
        if (cancelled) return
        setCount(inbox.unread_count)

        if ('Notification' in window && Notification.permission === 'granted') {
          const seen = new Set(readSeen())
          const notify = inbox.notifications
            .filter((item) => !item.read_at && !seen.has(item.id))
            .filter((item) => item.severity === 'CRITICAL' || item.severity === 'WARNING')
            .slice(0, firstLoad.current ? 0 : 3)
          for (const item of notify) {
            new Notification(`Argus · ${item.title}`, { body: `${item.project_name}: ${item.message}`, tag: `argus:${item.id}` })
            seen.add(item.id)
          }
          for (const item of inbox.notifications.slice(0, 100)) seen.add(item.id)
          window.localStorage.setItem(SEEN_KEY, JSON.stringify([...seen].slice(-250)))
        }
        firstLoad.current = false
      } catch (error) {
        console.error('Unable to refresh notification indicator', error)
      }
    }

    void refresh()
    const timer = window.setInterval(() => void refresh(), 60_000)
    const onVisibility = () => { if (document.visibilityState === 'visible') void refresh() }
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      cancelled = true
      window.clearInterval(timer)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [])

  if (count <= 0) return null
  return <span className="notification-count" aria-label={`${count} unread notification${count === 1 ? '' : 's'}`}>{count > 99 ? '99+' : count}</span>
}
