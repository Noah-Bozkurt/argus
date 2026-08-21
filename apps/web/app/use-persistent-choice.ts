'use client'

import { useEffect, useState } from 'react'

export default function usePersistentChoice<T extends string>(key: string, fallback: T, allowed: readonly T[]): [T, (value: T) => void] {
  const [value, setValue] = useState<T>(fallback)

  useEffect(() => {
    const stored = window.localStorage.getItem(key) as T | null
    if (stored && allowed.includes(stored)) setValue(stored)
  }, [allowed, key])

  function update(next: T) {
    setValue(next)
    window.localStorage.setItem(key, next)
  }

  return [value, update]
}
