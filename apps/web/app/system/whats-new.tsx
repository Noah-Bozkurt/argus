'use client'

import { useEffect, useState } from 'react'

const KEY = 'argus:whats-new:last-seen:v1'

const highlights = [
  'Search or jump anywhere with Cmd/Ctrl+K or /.',
  'Pin projects and servers, with recent resources kept in the palette.',
  'Filter and sort projects, servers and jobs; choices are remembered.',
  'Live server operations now support log search, copy and download.',
  'Unread alerts can surface through the browser when permission is enabled.',
]

export default function WhatsNew({ revision }: { revision: string }) {
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    if (!revision || revision === 'unknown') return
    setVisible(window.localStorage.getItem(KEY) !== revision)
  }, [revision])

  if (!visible) return null

  return (
    <section className="detail-card whats-new-card">
      <div className="detail-card-header">
        <div><h2>What&apos;s new</h2><p>Interface changes available in this Argus revision.</p></div>
        <button className="small" type="button" onClick={() => { window.localStorage.setItem(KEY, revision); setVisible(false) }}>Got it</button>
      </div>
      <div className="detail-card-body">
        <ul className="compact-list">{highlights.map((highlight) => <li key={highlight}>{highlight}</li>)}</ul>
      </div>
    </section>
  )
}
