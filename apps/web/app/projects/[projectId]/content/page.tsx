import Link from 'next/link'

import { getContentWorkspace, getFormsWorkspace, getMediaLibrary, type ContentField } from '../../../../lib/content-api'
import { createContentModelAction, deleteMediaAction, saveContentRecordAction, updateMediaAction, uploadMediaAction } from './actions'
import { PageLayoutEditor } from './page-layout-editor'
import { FormsSection } from './forms-section'

function FieldInput({ field, value, records = [], selected = [], media = [] }: { field: ContentField; value?: unknown; records?: Awaited<ReturnType<typeof getContentWorkspace>>['records']; selected?: string[]; media?: Awaited<ReturnType<typeof getMediaLibrary>> }) {
  const name = `value_${field.key}`
  if (field.type === 'relationship') return <select name={`relation_${field.key}`} multiple={field.has_many} required={field.required} defaultValue={selected}>
    {!field.required && !field.has_many ? <option value="">None</option> : null}
    {records.filter((record) => record.model_id === field.target_model_id).map((record) => <option key={record.id} value={record.id}>{String(record.values.title ?? record.values.name ?? record.values.slug ?? record.id)}</option>)}
  </select>
  if (field.type === 'media') {
    const mediaSelected = Array.isArray(value) ? value.map(String) : value ? [String(value)] : []
    return <select name={name} multiple={field.has_many} required={field.required} defaultValue={mediaSelected}>
      {!field.required && !field.has_many ? <option value="">None</option> : null}
      {media.map((asset) => <option key={asset.id} value={asset.id}>{asset.alt} — {asset.filename}{asset.public_read ? ' — public' : ' — private'}</option>)}
    </select>
  }
  if (field.type === 'textarea' || field.type === 'json') return <textarea name={name} required={field.required} defaultValue={field.type === 'json' && value !== undefined ? JSON.stringify(value, null, 2) : String(value ?? '')} />
  if (field.type === 'boolean') return <input name={name} type="checkbox" defaultChecked={value === true} />
  const type = field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : field.type === 'datetime' ? 'datetime-local' : 'text'
  return <input name={name} type={type} required={field.required} defaultValue={String(value ?? '')} />
}

function RecordForm({ projectId, model, record, components, workspace, media }: {
  projectId: string
  model: Awaited<ReturnType<typeof getContentWorkspace>>['models'][number]
  record?: Awaited<ReturnType<typeof getContentWorkspace>>['records'][number]
  components: Awaited<ReturnType<typeof getContentWorkspace>>['models']
  workspace: Awaited<ReturnType<typeof getContentWorkspace>>
  media: Awaited<ReturnType<typeof getMediaLibrary>>
}) {
  return (
    <form action={async (formData) => { 'use server'; await saveContentRecordAction(projectId, model.fields, formData) }}>
      <input type="hidden" name="model_id" value={model.id} />
      <input type="hidden" name="record_id" value={record?.id ?? ''} />
      <div className="form-grid">
        {model.fields.map((field) => (
          <label className={field.type === 'textarea' || field.type === 'json' ? 'full' : undefined} key={field.key}>
            {field.label}{field.required ? ' *' : ''}
            <FieldInput field={field} value={record?.values[field.key]} records={workspace.records} media={media} selected={record ? workspace.relations.filter((relation) => relation.source_record_id === record.id && relation.field_key === field.key).map((relation) => relation.target_record_id) : []} />
          </label>
        ))}
      </div>
      {model.content_role === 'page' ? <PageLayoutEditor components={components} initialLayout={record?.layout ?? []} media={media} /> : <input type="hidden" name="layout" value="[]" />}
      <div className="action-row"><button type="submit" name="intent" value="draft">Save draft</button><button className="primary" type="submit" name="intent" value="publish">Publish</button></div>
    </form>
  )
}

export default async function ProjectContentPage({ params, searchParams }: { params: { projectId: string }; searchParams: { submission_page?: string } }) {
  const parsedPage = Number.parseInt(searchParams.submission_page ?? '1', 10)
  const submissionPage = Number.isFinite(parsedPage) ? Math.max(1, parsedPage) : 1
  const [workspace, media, forms] = await Promise.all([getContentWorkspace(params.projectId), getMediaLibrary(params.projectId), getFormsWorkspace(params.projectId, submissionPage)])
  const componentModels = workspace.models.filter((model) => model.content_role === 'component')
  const publicBase = process.env.ARGUS_CONTENT_PUBLIC_URL?.replace(/\/$/, '')
  const publishedRecords = workspace.records.filter((record) => record.editorial_status === 'published').length

  return (
    <main className="detail-page">
      <div className="page-header">
        <div>
          <Link className="panel-link" href={`/projects/${params.projectId}`}>← Project</Link>
          <div style={{ marginTop: 10 }}><span className="eyebrow">Content workspace</span><h1>Content</h1><p>Model content, manage media and forms, then draft or publish project-owned records.</p></div>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-card"><div className="stat-label">Content types</div><div className="stat-value">{workspace.models.length}</div><div className="stat-meta">Schemas in this project</div></div>
        <div className="stat-card"><div className="stat-label">Records</div><div className="stat-value">{workspace.records.length}</div><div className="stat-meta">{publishedRecords} published</div></div>
        <div className="stat-card"><div className="stat-label">Media</div><div className="stat-value">{media.length}</div><div className="stat-meta">Project-owned image assets</div></div>
        <div className="stat-card"><div className="stat-label">Forms</div><div className="stat-value">{forms.forms.length}</div><div className="stat-meta">Public submission endpoints</div></div>
      </div>

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Media library</h2><p>Images up to 10 MiB with generated thumbnail, medium and large variants. Public delivery is explicit.</p></div><span className="badge">{media.length} assets</span></div>
        <div className="detail-card-body">
          <details className="create-drawer">
            <summary className="button">+ Upload image</summary>
            <div className="drawer-content">
              <form action={async (formData) => { 'use server'; await uploadMediaAction(params.projectId, formData) }}>
                <div className="form-grid">
                  <label>Image<input name="file" type="file" accept="image/jpeg,image/png,image/webp,image/avif" required /></label>
                  <label>Alternative text<input name="alt" required maxLength={300} /></label>
                  <label className="full">Caption<textarea name="caption" maxLength={2000} /></label>
                  <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="public_read" type="checkbox" value="true" /> Allow public delivery</label>
                </div>
                <button className="primary" type="submit">Upload image</button>
              </form>
            </div>
          </details>

          {media.length === 0 ? <div className="empty-state"><strong>No media</strong>Upload the first image for this project.</div> : (
            <div className="resource-list">
              {media.map((asset) => (
                <article className="resource-card" key={asset.id}>
                  <div className="resource-card-head">
                    <div><h3>{asset.alt}</h3><div className="resource-meta">{asset.filename} · {asset.width ?? '?'}×{asset.height ?? '?'} · {Math.ceil(asset.filesize / 1024)} KiB</div></div>
                    <span className={`badge ${asset.public_read ? 'success' : ''}`}>{asset.public_read ? 'Public' : 'Private'}</span>
                  </div>
                  {asset.caption ? <div className="resource-meta">{asset.caption}</div> : null}
                  <div className="action-row">
                    {asset.public_read && asset.url && publicBase ? <a className="button small" href={`${publicBase}${asset.url}`} target="_blank" rel="noreferrer">View image ↗</a> : null}
                    <details className="resource-editor">
                      <summary className="button small">Edit asset</summary>
                      <div className="resource-editor-body">
                        <form action={async (formData) => { 'use server'; await updateMediaAction(params.projectId, asset.id, formData) }}>
                          <label>Alternative text<input name="alt" required maxLength={300} defaultValue={asset.alt} /></label>
                          <label>Caption<textarea name="caption" maxLength={2000} defaultValue={asset.caption} /></label>
                          <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="public_read" type="checkbox" defaultChecked={asset.public_read} /> Allow public delivery</label>
                          <button type="submit">Save media details</button>
                        </form>
                        <form action={async (formData) => { 'use server'; await deleteMediaAction(params.projectId, asset.id, formData) }}>
                          <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion of original and variants</label>
                          <button className="danger" type="submit">Delete media</button>
                        </form>
                      </div>
                    </details>
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>
      </section>

      <FormsSection projectId={params.projectId} workspace={forms} publicBase={publicBase} />

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Content types &amp; records</h2><p>Project schemas and their draft/published content.</p></div><span className="badge">{workspace.models.length} types</span></div>
        <div className="detail-card-body">
          <details className="create-drawer">
            <summary className="button">+ New content type</summary>
            <div className="drawer-content">
              <form action={async (formData) => { 'use server'; await createContentModelAction(params.projectId, formData) }}>
                <div className="form-grid">
                  <label>Name<input name="name" required maxLength={160} placeholder="Articles" /></label>
                  <label>API slug<input name="slug" required pattern="[a-z][a-z0-9_]*" maxLength={120} placeholder="articles" /></label>
                  <label>Purpose<select name="content_role" defaultValue="collection"><option value="collection">Collection — repeatable entries</option><option value="page">Page — fields plus component layout</option><option value="component">Component — reusable page block</option></select></label>
                  <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input type="checkbox" name="public_read" /> Public when published</label>
                  <label className="full">Description<textarea name="description" maxLength={4000} /></label>
                </div>
                <fieldset>
                  <legend>Components allowed in pages</legend>
                  {componentModels.length === 0 ? <div className="muted">No component schemas exist yet.</div> : <div className="chip-list">{componentModels.map((component) => <label className="chip" key={component.id}><input type="checkbox" name="allowed_component_ids" value={component.id} /> {component.name}</label>)}</div>}
                </fieldset>
                <fieldset>
                  <legend>Fields</legend>
                  <div className="resource-list">
                    {[0, 1, 2, 3, 4].map((index) => (
                      <div className="resource-card" key={index}>
                        <div className="form-grid">
                          <label>Label<input name={`field_${index}_label`} placeholder={index === 0 ? 'Title' : 'Field label'} required={index === 0} /></label>
                          <label>Key<input name={`field_${index}_key`} placeholder={index === 0 ? 'title' : 'field_key'} pattern="[a-z][a-z0-9_]*" required={index === 0} /></label>
                          <label>Type<select name={`field_${index}_type`} defaultValue={index === 1 ? 'textarea' : 'text'}><option value="text">Short text</option><option value="textarea">Long text</option><option value="number">Number</option><option value="boolean">Yes / no</option><option value="date">Date</option><option value="datetime">Date and time</option><option value="json">Structured JSON</option><option value="relationship">Relationship</option><option value="media">Media image</option></select></label>
                          <label>Relationship target<select name={`field_${index}_target_model_id`} defaultValue=""><option value="">Not applicable</option>{workspace.models.filter((candidate) => candidate.content_role !== 'component').map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.name}</option>)}</select></label>
                          <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input type="checkbox" name={`field_${index}_required`} defaultChecked={index === 0} /> Required</label>
                          <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input type="checkbox" name={`field_${index}_has_many`} /> Multiple selections</label>
                        </div>
                      </div>
                    ))}
                  </div>
                </fieldset>
                <button className="primary" type="submit">Create content type</button>
              </form>
            </div>
          </details>

          {workspace.models.length === 0 ? <div className="empty-state"><strong>No content types</strong>Create a schema for pages, collections or reusable components.</div> : (
            <div className="resource-list">
              {workspace.models.map((model) => {
                const records = workspace.records.filter((record) => record.model_id === model.id)
                return (
                  <article className="resource-card" key={model.id}>
                    <div className="resource-card-head"><div><h3>{model.name}</h3><div className="resource-meta">{model.description || 'No description'} · <code>{model.slug}</code> · schema v{model.schema_version}</div></div><div className="action-row"><span className="badge info">{model.content_role}</span><span className={`badge ${model.public_read ? 'success' : ''}`}>{model.public_read ? 'Public published records' : 'Private'}</span><span className="badge">{records.length} records</span></div></div>
                    {model.content_role === 'component' ? <div className="callout" style={{ marginTop: 12 }}>Reusable block schema available to page types that allow it.</div> : (
                      <>
                        <details className="resource-editor">
                          <summary className="button small">+ New record</summary>
                          <div className="resource-editor-body"><RecordForm projectId={params.projectId} model={model} components={componentModels.filter((component) => model.allowed_component_ids.includes(component.id))} workspace={workspace} media={media} /></div>
                        </details>
                        {records.length === 0 ? <div className="empty-state"><strong>No records</strong>Create the first {model.name.toLowerCase()} record.</div> : (
                          <div className="resource-list" style={{ marginTop: 12 }}>
                            {records.map((record) => {
                              const label = String(record.values.title ?? record.values.name ?? record.values.slug ?? record.id)
                              return (
                                <article className="resource-card" key={record.id}>
                                  <div className="resource-card-head"><div><h4>{label}</h4><div className="resource-meta">{record.published_at ? `Published ${new Date(record.published_at).toLocaleString()}` : 'Not published yet'}</div></div><span className={`badge ${record.editorial_status === 'published' ? 'success' : 'warning'}`}>{record.editorial_status}</span></div>
                                  <div className="action-row"><Link className="button small" href={`/projects/${params.projectId}/content/preview/${record.id}`}>Preview</Link><details className="resource-editor"><summary className="button small">Edit record</summary><div className="resource-editor-body"><RecordForm projectId={params.projectId} model={model} record={record} components={componentModels.filter((component) => model.allowed_component_ids.includes(component.id))} workspace={workspace} media={media} /></div></details></div>
                                </article>
                              )
                            })}
                          </div>
                        )}
                      </>
                    )}
                  </article>
                )
              })}
            </div>
          )}
        </div>
      </section>
    </main>
  )
}
