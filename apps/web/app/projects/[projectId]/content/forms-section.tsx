import Link from 'next/link'

import type { FormsWorkspace } from '../../../../lib/content-api'
import { createProjectFormAction, deleteFormSubmissionAction, updateFormSubmissionStatusAction, updateProjectFormStatusAction } from './actions'
import { FormFieldsEditor } from './form-fields-editor'

function statusClass(status: string): string {
  if (status === 'published' || status === 'reviewed') return 'success'
  if (status === 'spam') return 'danger'
  if (status === 'new' || status === 'draft') return 'warning'
  return ''
}

export function FormsSection({ projectId, workspace, publicBase }: { projectId: string; workspace: FormsWorkspace; publicBase?: string }) {
  return (
    <section className="detail-card">
      <div className="detail-card-header"><div><h2>Forms &amp; submissions</h2><p>Typed public form endpoints with private, durably rate-limited submissions.</p></div><span className="badge">{workspace.forms.length} forms</span></div>
      <div className="detail-card-body">
        <details className="create-drawer">
          <summary className="button">+ Create form</summary>
          <div className="drawer-content">
            <form action={async (formData) => { 'use server'; await createProjectFormAction(projectId, formData) }}>
              <div className="form-grid">
                <label>Name<input name="form_name" required maxLength={160} placeholder="Contact" /></label>
                <label>API slug<input name="form_slug" required pattern="[a-z][a-z0-9_]*" maxLength={120} placeholder="contact" /></label>
                <label>Success message<input name="form_success_message" required maxLength={500} defaultValue="Thanks — your response was received." /></label>
                <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input type="checkbox" name="form_published" /> Publish immediately</label>
                <label className="full">Description<textarea name="form_description" maxLength={2000} /></label>
              </div>
              <FormFieldsEditor />
              <button className="primary" type="submit">Create form</button>
            </form>
          </div>
        </details>

        {workspace.forms.length === 0 ? <div className="empty-state"><strong>No forms</strong>Create a form when this project needs a public submission endpoint.</div> : (
          <div className="resource-list">
            {workspace.forms.map((form) => {
              const submissions = workspace.submissions.filter((submission) => submission.form_id === form.id)
              const endpoint = publicBase ? `${publicBase}/public/projects/${encodeURIComponent(projectId)}/forms/${encodeURIComponent(form.slug)}` : null
              return (
                <article className="resource-card" key={form.id}>
                  <div className="resource-card-head">
                    <div><h3>{form.name}</h3><div className="resource-meta">{form.description || 'No description'} · <code>{form.slug}</code></div></div>
                    <div className="action-row"><span className={`badge ${statusClass(form.status)}`}>{form.status}</span><span className="badge">{submissions.length} submissions</span></div>
                  </div>

                  <div className="detail-hero-meta">{form.fields.map((field) => <span className="badge" key={field.key}>{field.label} · {field.type}{field.required ? ' *' : ''}</span>)}</div>
                  <div className="action-row">
                    {endpoint ? <a className="button small" href={endpoint} target="_blank" rel="noreferrer">Open endpoint ↗</a> : null}
                    <a className="button small" href={`/projects/${encodeURIComponent(projectId)}/content/forms/${encodeURIComponent(form.id)}/submissions.csv`}>Download CSV</a>
                    {(['draft', 'published', 'archived'] as const).filter((status) => status !== form.status).map((status) => <form key={status} action={async () => { 'use server'; await updateProjectFormStatusAction(projectId, form.id, status) }}><button className="small" type="submit">Set {status}</button></form>)}
                  </div>

                  <details className="log-details" style={{ marginTop: 12 }}>
                    <summary>Submissions · {submissions.length}</summary>
                    <div className="resource-list" style={{ padding: 10 }}>
                      {submissions.length === 0 ? <div className="empty-state"><strong>No submissions</strong>New responses will appear here.</div> : submissions.map((submission) => (
                        <article className="resource-card" key={submission.id}>
                          <div className="resource-card-head"><div><strong>{submission.submitted_at ? new Date(submission.submitted_at).toLocaleString() : 'Unknown time'}</strong></div><span className={`badge ${statusClass(submission.status)}`}>{submission.status}</span></div>
                          <div className="info-grid" style={{ marginTop: 10 }}>{form.fields.map((field) => <div className="info-item" key={field.key}><span className="info-label">{field.label}</span><span className="info-value">{typeof submission.values[field.key] === 'object' ? JSON.stringify(submission.values[field.key]) : String(submission.values[field.key] ?? '—')}</span></div>)}</div>
                          <div className="action-row">{(['new', 'reviewed', 'spam', 'archived'] as const).filter((status) => status !== submission.status).map((status) => <form key={status} action={async () => { 'use server'; await updateFormSubmissionStatusAction(projectId, submission.id, status) }}><button className="small" type="submit">Mark {status}</button></form>)}</div>
                          <details className="resource-editor">
                            <summary className="button small danger">Delete submission</summary>
                            <div className="resource-editor-body"><form action={async (formData) => { 'use server'; await deleteFormSubmissionAction(projectId, submission.id, formData) }}><label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion</label><button className="danger" type="submit">Delete permanently</button></form></div>
                          </details>
                        </article>
                      ))}
                    </div>
                  </details>
                </article>
              )
            })}
          </div>
        )}

        {workspace.submission_pagination.total_docs > 0 ? (
          <nav className="action-row" aria-label="Submission pages">
            <span className="muted">Page {workspace.submission_pagination.page}/{workspace.submission_pagination.total_pages} · {workspace.submission_pagination.total_docs} total</span>
            {workspace.submission_pagination.has_prev_page ? <Link className="button small" href={`/projects/${projectId}/content?submission_page=${workspace.submission_pagination.page - 1}`}>← Newer</Link> : null}
            {workspace.submission_pagination.has_next_page ? <Link className="button small" href={`/projects/${projectId}/content?submission_page=${workspace.submission_pagination.page + 1}`}>Older →</Link> : null}
          </nav>
        ) : null}
      </div>
    </section>
  )
}
