import type { ReactNode } from 'react'

// The operator UI reads runtime-only Argus credentials and live control-plane data.
// Never prerender it into the image at build time.
export const dynamic = 'force-dynamic'

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  )
}
