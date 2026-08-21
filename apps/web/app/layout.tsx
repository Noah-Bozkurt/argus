import type { ReactNode } from 'react'
import { currentSessionToken, getWorkspaceUser } from '../lib/auth'
import AppShell from './app-shell'
import './globals.css'
import './feature-panels.css'
import './ui-polish.css'
import './ui-resource-editors.css'
import './command-palette.css'
import './auth.css'

// The operator UI reads runtime-only Argus credentials and live control-plane data.
// Never prerender it into the image at build time.
export const dynamic = 'force-dynamic'

export const metadata = {
  title: 'Argus Control Plane',
  description: 'Projects, infrastructure and operations in one control plane.',
}

export default async function RootLayout({ children }: { children: ReactNode }) {
  const token = currentSessionToken()
  const user = token ? await getWorkspaceUser(token) : null

  return (
    <html lang="en">
      <body>
        <AppShell user={user}>{children}</AppShell>
      </body>
    </html>
  )
}
