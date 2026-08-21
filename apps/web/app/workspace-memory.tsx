'use client'

import { useEffect } from 'react'
import { usePathname } from 'next/navigation'

function storageKey(pathname: string): string | null {
  if (/^\/projects\/[^/]+$/.test(pathname)) return `argus:last-section:${pathname}`
  if (/^\/infrastructure\/servers\/[^/]+$/.test(pathname)) return `argus:last-section:${pathname}`
  return null
}

export default function WorkspaceMemory() {
  const pathname = usePathname()

  useEffect(() => {
    const key = storageKey(pathname)
    if (!key) return

    const save = () => {
      if (window.location.hash) window.localStorage.setItem(key, window.location.hash)
    }

    if (!window.location.hash) {
      const remembered = window.localStorage.getItem(key)
      if (remembered && document.querySelector(remembered)) {
        window.history.replaceState(window.history.state, '', `${pathname}${remembered}`)
        window.requestAnimationFrame(() => document.querySelector(remembered)?.scrollIntoView({ block: 'start' }))
      }
    } else {
      save()
    }

    window.addEventListener('hashchange', save)
    return () => window.removeEventListener('hashchange', save)
  }, [pathname])

  return null
}
