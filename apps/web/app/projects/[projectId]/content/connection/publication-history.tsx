'use client'
import { useRef, useState } from 'react'
import { useRouter } from 'next/navigation'
import type { SiteConnectionView } from '../../../../../lib/content-api'
export function PublicationHistory({ projectId, releases }: { projectId: string; releases: SiteConnectionView['releases'] }) {
 const [busy, setBusy] = useState(false)
 const [message, setMessage] = useState('')
 const requests = useRef(new Map<string, string>())
 const router = useRouter()
 async function act(releaseId?: string) {
  setBusy(true)
  if (releaseId && !requests.current.has(releaseId)) requests.current.set(releaseId, crypto.randomUUID())
  try {
   const response = await fetch(`/api/projects/${projectId}/site`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action: 'retry', ...(releaseId ? { releaseId, requestId: requests.current.get(releaseId) } : { operation: 'reconcile' }) }) })
   const result = await response.json()
   setMessage(response.ok ? (releaseId ? 'Retry queued.' : 'Deployment status checked.') : result.code)
   router.refresh()
  } catch { setMessage('Unable to confirm the request. Try again.') } finally { setBusy(false) }
 }
 return <section className="panel"><h2>Publication history</h2><button disabled={busy} onClick={() => act()}>Check deployment status</button>{releases.map(release => <div key={release.id}><p>{new Date(release.createdAt).toLocaleString()} · <strong>{release.status}</strong>{release.errorCode ? ` · ${release.errorCode}` : ''}</p>{release.status === 'failed' && <button disabled={busy} onClick={() => act(release.id)}>Retry this release</button>}</div>)}{!releases.length && <p>No publications yet.</p>}<p role="status">{message}</p></section>
}
