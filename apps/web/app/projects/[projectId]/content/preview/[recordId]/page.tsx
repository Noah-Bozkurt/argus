import Link from 'next/link'
import { notFound } from 'next/navigation'

import { getContentWorkspace, type ContentField } from '../../../../../../lib/content-api'

function PreviewValue({ field, value }: { field: ContentField; value: unknown }) {
  if (value === null || value === undefined || value === '') return <span className="muted">Not set</span>
  if (field.type === 'boolean') return <span>{value === true ? 'Yes' : 'No'}</span>
  if (field.type === 'json') return <pre>{JSON.stringify(value, null, 2)}</pre>
  if (field.type === 'date' || field.type === 'datetime') {
    const parsed = new Date(String(value))
    return <span>{Number.isNaN(parsed.getTime()) ? String(value) : parsed.toLocaleString()}</span>
  }
  if (field.type === 'textarea') return <span style={{ whiteSpace: 'pre-wrap' }}>{String(value)}</span>
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
  const title = String(record.values.title ?? record.values.name ?? record.values.slug ?? `${model.name} record`)

  return (
    <main className="detail-page">
      <div className="page-header">
        <div>
          <Link className="panel-link" href={`/projects/${params.projectId}/content`}>← Content</Link>
          <div style={{ marginTop: 10 }}><span className="eyebrow">{model.name} preview</span><h1>{title}</h1></div>
          <div className="detail-hero-meta"><span className={`badge ${record.editorial_status === 'published' ? 'success' : 'warning'}`}>{record.editorial_status}</span><span className="badge">Schema v{model.schema_version}</span>{record.updated_at ? <span className="badge">Updated {new Date(record.updated_at).toLocaleString()}</span> : null}</div>
        </div>
        {publicUrl ? <div className="page-actions"><a className="button primary" href={publicUrl} target="_blank" rel="noreferrer">View public response ↗</a></div> : null}
      </div>

      {record.editorial_status === 'draft' ? <div className="callout warning">This draft preview is visible only inside the protected Argus interface.</div> : null}

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Fields</h2><p>Stored values for this content record.</p></div></div>
        <div className="detail-card-body">
          <div className="info-grid">
            {model.fields.map((field) => <div className="info-item" key={field.key}><span className="info-label">{field.label}</span><span className="info-value"><PreviewValue field={field} value={record.values[field.key]} /></span></div>)}
          </div>
        </div>
      </section>

      {model.content_role === 'page' ? (
        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Page layout</h2><p>{record.layout.length} component blocks in rendered order.</p></div></div>
          <div className="detail-card-body">
            {record.layout.length === 0 ? <div className="empty-state"><strong>No layout blocks</strong>This page has no component layout yet.</div> : <div className="resource-list">{record.layout.map((block, index) => {
              const component = workspace.models.find((candidate) => candidate.content_role === 'component' && candidate.slug === block.component)
              return <article className="resource-card" key={block.id}><div className="resource-card-head"><h3>{component?.name ?? block.component}</h3><span className="badge">Block {index + 1}</span></div>{component ? <div className="info-grid" style={{ marginTop: 10 }}>{component.fields.map((field) => <div className="info-item" key={field.key}><span className="info-label">{field.label}</span><span className="info-value"><PreviewValue field={field} value={block.values[field.key]} /></span></div>)}</div> : <div className="callout warning" style={{ marginTop: 10 }}>This component schema is unavailable.</div>}</article>
            })}</div>}
          </div>
        </section>
      ) : null}
    </main>
  )
}
