'use client'

import { useState, type FormEvent } from 'react'

export default function AddServerSection({ projectId, environments }: { projectId: string; environments: Array<{ id: string; name: string }> }) {
  const [setupCode, setSetupCode] = useState('')
  const [error, setError] = useState('')

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setError('')
    setSetupCode('')
    const form = new FormData(event.currentTarget)
    const response = await fetch(`/api/projects/${projectId}/servers`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ environment_id: form.get('environment_id'), hostname: form.get('hostname') }),
    })
    const body = await response.json()
    if (!response.ok) setError(body.message ?? 'Could not create server.')
    else setSetupCode(body.setup_code)
  }

  return (
    <section>
      <h2>Managed servers</h2>
      <p>Enroll a node with a 15-minute, single-use setup code. The enrollment secret is only returned once.</p>

      <h3>Server enrollment</h3>
      <details className="create-drawer">
        <summary className="button">+ Add server</summary>
        <div className="drawer-content">
          {environments.length === 0 ? <div className="callout warning">Create an environment before enrolling a server.</div> : (
            <form onSubmit={submit}>
              <div className="form-grid">
                <label>Environment<select name="environment_id" required defaultValue=""><option value="" disabled>Select environment</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
                <label>Hostname<input name="hostname" required maxLength={255} autoComplete="off" placeholder="web-01" /></label>
              </div>
              <button className="primary" type="submit">Create setup code</button>
            </form>
          )}
        </div>
      </details>

      {error ? <div className="callout danger" role="alert">{error}</div> : null}
      {setupCode ? (
        <div className="resource-card" style={{ margin: '0 17px 17px' }}>
          <div className="resource-card-head"><div><h4>Enrollment code ready</h4><div className="resource-meta">Single use · expires in 15 minutes · shown only now</div></div><span className="badge warning">Secret</span></div>
          <div className="callout warning" style={{ marginTop: 12 }}>Copy this code before leaving the page. Argus will not show it again.</div>
          <label style={{ marginTop: 12 }}>Setup code<input type="password" readOnly value={setupCode} aria-label="Setup code" onFocus={(event) => event.currentTarget.select()} /></label>
          <div className="log-details" style={{ marginTop: 12 }}><div style={{ padding: 12 }}><span className="info-label">Install command</span><code>curl -fsSL https://argus-installer.pages.dev/install | sudo bash</code></div></div>
        </div>
      ) : null}
    </section>
  )
}
