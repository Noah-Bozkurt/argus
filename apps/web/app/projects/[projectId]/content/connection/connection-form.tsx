'use client'
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import type { SiteConnectionView } from '../../../../../lib/content-api'
export function ConnectionForm({ projectId, site }: { projectId: string; site: SiteConnectionView['site'] }) {
  const [targets, setTargets] = useState<Array<{ provider: string; name: string; branch: string }>>([])
  const [message, setMessage] = useState('')
  const [busy, setBusy] = useState(false)
  const router = useRouter()
  async function submit(form: HTMLFormElement, operation: string) {
    setBusy(true); setMessage('')
    const body = Object.fromEntries(new FormData(form))
    const selected = targets.find(target => `${target.provider}:${target.name}` === body.selection)
    try {
      const response = await fetch(`/api/projects/${projectId}/site`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...body, action: 'configure', operation, ...(selected ? { provider: selected.provider, target: selected.name } : {}) }) })
      const result = await response.json()
      if (!response.ok) { setMessage(result.code); return }
      if (operation === 'discover') { setTargets(result.targets); setMessage('Select your site, then connect it.') }
      else { form.reset(); setTargets([]); setMessage('Site connected.'); router.refresh() }
    } catch { setMessage('Unable to connect. Try again.') } finally { setBusy(false) }
  }
  return <form onSubmit={event => { event.preventDefault(); void submit(event.currentTarget, 'connect') }}>
    <label>Website URL<input type="url" name="siteURL" required defaultValue={site?.siteURL} placeholder="https://your-site.example" /></label>
    <label>Preview URL<input type="url" name="previewURL" defaultValue={site?.previewURL ?? ''} placeholder="https://your-preview.example/argus/preview" /></label>
    <label>Cloudflare account ID<input name="accountId" required defaultValue={site?.accountId} pattern="[a-fA-F0-9]{32}" /></label>
    <label>Cloudflare API token<input type="password" name="token" required autoComplete="off" /></label>
    <p>Use a token scoped to this account with Pages Edit or Workers CI Write and Workers Scripts Read. Credentials remain server-side.</p>
    <button type="button" disabled={busy} onClick={event => { const form = event.currentTarget.form; if (form) void submit(form, 'discover') }}>Find Cloudflare sites</button>
    <label>Deployment target<select name="selection" required>{targets.length ? targets.map(target => <option key={`${target.provider}:${target.name}`} value={`${target.provider}:${target.name}`}>{target.name} ({target.provider})</option>) : <option value="">Find sites first</option>}</select></label>
    <label>Production branch<input name="branch" required defaultValue={site?.branch ?? ''} placeholder="Your production branch" /></label>
    <p>Workers must already have a Git build configured. Connecting creates a deploy hook but does not deploy your site.</p>
    <button className="primary" disabled={busy || !targets.length}>Connect site</button><p role="status">{message}</p>
  </form>
}
