'use client'

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import * as Tooltip from '@radix-ui/react-tooltip'
import { useState, type ReactNode } from 'react'

export default function AppProviders({ children }: { children: ReactNode }) {
  const [queryClient] = useState(() => new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 5_000,
        retry: 1,
        refetchOnWindowFocus: true,
      },
    },
  }))

  return (
    <QueryClientProvider client={queryClient}>
      <Tooltip.Provider delayDuration={300} skipDelayDuration={100}>
        {children}
      </Tooltip.Provider>
    </QueryClientProvider>
  )
}
