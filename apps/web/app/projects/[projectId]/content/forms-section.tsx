import Link from 'next/link'

import type { FormsWorkspace } from '../../../../lib/content-api'
import { createProjectFormAction, deleteFormSubmissionAction, updateFormSubmissionStatusAction, updateProjectFormStatusAction } from './actions'
import { FormFieldsEditor } from './form-fields-editor'

export function FormsSection({ projectId, workspace, publicBase }: { projectId: string; workspace: FormsWorkspace; publicBase?: string }) {
  return <section>
    <h2>Forms and submissions</h2>
    <p>Create a typed public form endpoint. Submissions remain private inside Argus and are durably rate-limited without storing raw source addresses.</p>
    <form action={async (formData) => { 'use server'; await createProjectFormAction(projectId, formData) }}>
      <label>Name<input name="form_name" required maxLength={160} placeholder="Contact" /></label>
      <label>API slug<input name="form_slug" required pattern="[a-z][a-z0-9_]*" maxLength={120} placeholder="contact" /></label>
      <label>Description<textarea name="form_description" maxLength={2000} /></label>
      <label>Success message<input name="form_success_message" required maxLength={500} defaultValue="Thanks — your response was received." /></label>
      <label><input type="checkbox" name="form_published" /> Publish and accept submissions immediately</label>
      <FormFieldsEditor />
      <button type="submit">Create form</button>
    </form>
    {workspace.forms.length === 0 ? <p>No forms yet.</p> : workspace.forms.map((form) => {
      const submissions = workspace.submissions.filter((submission) => submission.form_id === form.id)
      const endpoint = publicBase ? `${publicBase}/public/projects/${encodeURIComponent(projectId)}/forms/${encodeURIComponent(form.slug)}` : null
      return <article key={form.id}>
        <h3>{form.name}</h3>
        <p>{form.description || 'No description.'} — {form.status} — slug <code>{form.slug}</code></p>
        {endpoint ? <p>Public endpoint: <a href={endpoint} target="_blank" rel="noreferrer">{endpoint}</a></p> : null}
        <p>Fields: {form.fields.map((field) => `${field.label} (${field.type}${field.required ? ', required' : ''})`).join(', ')}</p>
        <p><a href={`/projects/${encodeURIComponent(projectId)}/content/forms/${encodeURIComponent(form.id)}/submissions.csv`}>Download submissions CSV</a></p>
        {(['draft', 'published', 'archived'] as const).filter((status) => status !== form.status).map((status) => <form key={status} action={async () => { 'use server'; await updateProjectFormStatusAction(projectId, form.id, status) }}><button type="submit">Set {status}</button></form>)}
        <h4>Submissions</h4>
        {submissions.length === 0 ? <p>No submissions.</p> : submissions.map((submission) => <section key={submission.id}>
          <p><strong>{submission.status}</strong>{submission.submitted_at ? ` — ${new Date(submission.submitted_at).toLocaleString()}` : ''}</p>
          <dl>{form.fields.map((field) => <div key={field.key}><dt>{field.label}</dt><dd>{typeof submission.values[field.key] === 'object' ? JSON.stringify(submission.values[field.key]) : String(submission.values[field.key] ?? '—')}</dd></div>)}</dl>
          {(['new', 'reviewed', 'spam', 'archived'] as const).filter((status) => status !== submission.status).map((status) => <form key={status} action={async () => { 'use server'; await updateFormSubmissionStatusAction(projectId, submission.id, status) }}><button type="submit">Mark {status}</button></form>)}
          <form action={async (formData) => { 'use server'; await deleteFormSubmissionAction(projectId, submission.id, formData) }}>
            <label><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion</label>
            <button type="submit">Delete submission</button>
          </form>
        </section>)}
      </article>
    })}
    {workspace.submission_pagination.total_docs > 0 ? <nav>
      <p>Showing submission page {workspace.submission_pagination.page} of {workspace.submission_pagination.total_pages} ({workspace.submission_pagination.total_docs} total).</p>
      {workspace.submission_pagination.has_prev_page ? <Link href={`/projects/${projectId}/content?submission_page=${workspace.submission_pagination.page - 1}`}>← Newer submissions</Link> : null}
      {' '}
      {workspace.submission_pagination.has_next_page ? <Link href={`/projects/${projectId}/content?submission_page=${workspace.submission_pagination.page + 1}`}>Older submissions →</Link> : null}
    </nav> : null}
  </section>
}
