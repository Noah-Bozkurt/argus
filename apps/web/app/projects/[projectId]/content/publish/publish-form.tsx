'use client'
import { useRef, useState } from 'react'
import { useRouter } from 'next/navigation'
export function PublishForm({ projectId, entries }: { projectId: string; entries: Array<{ id: string; label: string; status: string }> }) {
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const requestId = useRef<string | null>(null)
  const router = useRouter()
  return <form onChange={() => { requestId.current = null }} onSubmit={async event => {
    event.preventDefault(); setBusy(true); setMessage('')
    const recordIds = new FormData(event.currentTarget).getAll('records')
    requestId.current ??= crypto.randomUUID()
    try {
      const response = await fetch(`/api/projects/${projectId}/site`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action: 'publish', requestId: requestId.current, recordIds }) })
      const body = await response.json()
      if (!response.ok) { setMessage(body.code); return }
      router.push(`/projects/${projectId}/content/connection`); router.refresh()
    } catch { setMessage('Publication could not be confirmed. Retry to check the same request.') } finally { setBusy(false) }
  }}><p>Select drafts to publish together. Previously published entries are included automatically. Publishing with no selection rebuilds the existing published content.</p><div className="stack">{entries.map(entry => <label className="panel" key={entry.id}><input name="records" type="checkbox" value={entry.id} />{entry.label}<span className="badge">{entry.status}</span></label>)}</div><button className="primary" disabled={busy}>Publish changes</button><p role="status">{message}</p></form>
}
