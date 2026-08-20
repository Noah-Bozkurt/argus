import type { ReactNode } from 'react'
import AppShell from './app-shell'
import './globals.css'

// The operator UI reads runtime-only Argus credentials and live control-plane data.
// Never prerender it into the image at build time.
export const dynamic = 'force-dynamic'

export const metadata = {
  title: 'Argus Control Plane',
  description: 'Projects, infrastructure and operations in one control plane.',
}

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <AppShell>{children}</AppShell>
      </body>
    </html>
  )
}
