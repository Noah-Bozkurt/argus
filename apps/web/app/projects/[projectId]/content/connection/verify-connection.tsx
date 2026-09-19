'use client'
import { useState } from 'react'
export function VerifyConnection({ projectId }: { projectId: string }) {
 const [result, setResult] = useState('')
 const [busy, setBusy] = useState(false)
 async function verify(register: boolean) {
  setBusy(true)
  try {
   const response = await fetch(`/api/projects/${projectId}/site`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action: 'verify', register }) })
   const data = await response.json()
   setResult(response.ok ? `Website connected. Missing components: ${data.missing.join(', ') || 'none'}. Components needing schema review: ${data.mismatched.join(', ') || 'none'}. Preview: ${data.preview}. Existing schemas were preserved.` : `Verification failed: ${data.code}`)
  } catch { setResult('Unable to verify the website.') } finally { setBusy(false) }
 }
 return <div><div className="action-row"><button disabled={busy} onClick={() => verify(false)}>Verify website connection</button><button disabled={busy} onClick={() => verify(true)}>Import missing component definitions</button></div><p role="status">{result}</p></div>
}
