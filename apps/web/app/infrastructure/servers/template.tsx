'use client'

import { useEffect, useTransition, type ReactNode } from 'react'
import { useRouter } from 'next/navigation'

const REFRESH_INTERVAL_MS = 5_000

export default function ServersTemplate({ children }: { children: ReactNode }) {
  const router = useRouter()
  const [, startTransition] = useTransition()

  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState !== 'visible') return
      startTransition(() => router.refresh())
    }

    const timer = window.setInterval(refresh, REFRESH_INTERVAL_MS)
    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') refresh()
    }

    document.addEventListener('visibilitychange', onVisibilityChange)
    return () => {
      window.clearInterval(timer)
      document.removeEventListener('visibilitychange', onVisibilityChange)
    }
  }, [router])

  return children
}
