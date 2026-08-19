import config from '@payload-config'
import '@payloadcms/next/css'
import type { ServerFunctionClient } from 'payload'
import { handleServerFunctions, RootLayout } from '@payloadcms/next/layouts'
import React from 'react'

import { importMap } from './admin/importMap.js'

type Args = {
  children: React.ReactNode
}

// Argus intentionally has two React applications in one pnpm workspace:
// apps/web is React 18 while Payload uses React 19. pnpm therefore keeps two
// valid React type identities. Next's production checker can surface that at
// this single package boundary even though the runtime ReactNode is unchanged.
// Keep the app-facing children type local and bridge only RootLayout's declared
// children type instead of weakening TypeScript checks for the Payload app.
type RootLayoutChildren = Parameters<typeof RootLayout>[0]['children']

const serverFunction: ServerFunctionClient = async (args) => {
  'use server'
  return handleServerFunctions({
    ...args,
    config,
    importMap,
  })
}

const Layout = ({ children }: Args) => (
  <RootLayout config={config} importMap={importMap} serverFunction={serverFunction}>
    {children as unknown as RootLayoutChildren}
  </RootLayout>
)

export default Layout
