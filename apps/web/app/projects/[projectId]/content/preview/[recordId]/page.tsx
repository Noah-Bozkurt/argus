import Link from 'next/link'
import { notFound } from 'next/navigation'

import { getContentWorkspace, type ContentField } from '../../../../../../lib/content-api'

function PreviewValue({ field, value }: { field: ContentField; value: unknown }) {
  if (value === null || value === undefined || value === '') return <span>Not set</span>
  if (field.type === 'boolean') return <span>{value === true ? 'Yes' : 'No'}</span>
  if (field.type === 'json') return <pre>{JSON.stringify(value, null, 2)}</pre>
  if (field.type === 'date' || field.type === 'datetime') {
    const parsed = new Date(String(value))
    return <span>{Number.isNaN(parsed.getTime()) ? String(value) : parsed.toLocaleString()}</span>
  }
  if (field.type === 'textarea') return <p style={{ whiteSpace: 'pre-wrap' }}>{String(value)}</p>
  return <span>{String(value)}</span>
}

export default async function ContentPreviewPage({ params }: { params: { projectId: string; recordId: string } }) {
  const workspace = await getContentWorkspace(params.projectId)
  const record = workspace.records.find((candidate) => candidate.id === params.recordId)
  if (!record) notFound()
  const model = workspace.models.find((candidate) => candidate.id === record.model_id)
  if (!model) notFound()

  const publicBase = process.env.ARGUS_CONTENT_PUBLIC_URL?.replace(/\/$/, '')
  const publicUrl = publicBase && model.public_read && record.editorial_status === 'published'
    ? `${publicBase}/public/projects/${encodeURIComponent(params.projectId)}/content/${encodeURIComponent(model.slug)}`
    : null

  return (
    <main>
      <p><Link href={`/projects/${params.projectId}/content`}>← Content</Link></p>
      <h1>{model.name} preview</h1>
      <p>
        <strong>{record.editorial_status === 'published' ? 'Published' : 'Draft preview'}</strong>
        {' — '}schema v{model.schema_version}
        {record.updated_at ? ` — updated ${new Date(record.updated_at).toLocaleString()}` : ''}
      </p>
      {record.editorial_status === 'draft' ? <p>This preview is visible only inside the protected Argus operator interface.</p> : null}
      {publicUrl ? <p><a href={publicUrl} target="_blank" rel="noreferrer">View published public response</a></p> : null}
      <article>
        {model.fields.map((field) => (
          <section key={field.key}>
            <h2>{field.label}</h2>
            <PreviewValue field={field} value={record.values[field.key]} />
          </section>
        ))}
      </article>
    </main>
  )
}
