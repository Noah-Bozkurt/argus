'use client'

import { useState, type FormEvent } from 'react'

export default function AddServerSection({ projectId, environments }: { projectId: string; environments: Array<{ id: string; name: string }> }) {
  const [setupCode, setSetupCode] = useState('')
  const [error, setError] = useState('')
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setError(''); setSetupCode('')
    const form = new FormData(event.currentTarget)
    const response = await fetch(`/api/projects/${projectId}/servers`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ environment_id: form.get('environment_id'), hostname: form.get('hostname') }) })
    const body = await response.json()
    if (!response.ok) setError(body.message ?? 'Could not create server.')
    else setSetupCode(body.setup_code)
  }
  return <section>
    <h2>Add server</h2>
    <p>Create a 15-minute, single-use setup code for a managed node.</p>
    <form onSubmit={submit}>
      <label>Environment<select name="environment_id" required defaultValue=""><option value="" disabled>Select environment</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
      <label>Hostname<input name="hostname" required maxLength={255} autoComplete="off" /></label>
      <button type="submit">Create setup code</button>
    </form>
    {error ? <p role="alert">{error}</p> : null}
    {setupCode ? <div><p>Copy this code now. It is shown only in this response and expires after 15 minutes.</p><input type="password" readOnly value={setupCode} aria-label="Setup code" onFocus={(event) => event.currentTarget.select()} /><p><code>curl -fsSL https://argus-installer.pages.dev/install | sudo bash</code></p></div> : null}
  </section>
}
