'use client'

import { useEffect } from 'react'

function guardedForm(target: EventTarget | null): HTMLFormElement | null {
  const element = target as HTMLElement | null
  const form = element?.closest('form')
  if (!form || form.hasAttribute('data-no-dirty-guard')) return null
  return form
}

export default function FormDirtyGuard() {
  useEffect(() => {
    let dirty = false

    const markDirty = (event: Event) => {
      if (guardedForm(event.target)) dirty = true
    }
    const clearDirty = (event: Event) => {
      if (guardedForm(event.target)) dirty = false
    }
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (!dirty) return
      event.preventDefault()
      event.returnValue = ''
    }
    const captureLink = (event: MouseEvent) => {
      if (!dirty || event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
      const target = event.target as HTMLElement | null
      const link = target?.closest('a[href]') as HTMLAnchorElement | null
      if (!link || link.target === '_blank' || link.href === window.location.href) return
      if (!window.confirm('You have unsaved changes. Leave this page anyway?')) event.preventDefault()
      else dirty = false
    }

    document.addEventListener('input', markDirty, true)
    document.addEventListener('change', markDirty, true)
    document.addEventListener('submit', clearDirty, true)
    document.addEventListener('click', captureLink, true)
    window.addEventListener('beforeunload', beforeUnload)
    return () => {
      document.removeEventListener('input', markDirty, true)
      document.removeEventListener('change', markDirty, true)
      document.removeEventListener('submit', clearDirty, true)
      document.removeEventListener('click', captureLink, true)
      window.removeEventListener('beforeunload', beforeUnload)
    }
  }, [])

  return null
}
