import Link from 'next/link'

import { getContentWorkspace, getFormsWorkspace, getMediaLibrary, type ContentField, type ContentModel } from '../../../../lib/content-api'
import { createContentModelAction, deleteContentModelAction, deleteContentRecordAction, deleteMediaAction, saveContentRecordAction, setContentModelStatusAction, setContentRecordStatusAction, updateContentModelAction, updateMediaAction, uploadMediaAction } from './actions'
import { ContentModelFieldsEditor } from './content-model-fields-editor'
import { PageLayoutEditor } from './page-layout-editor'
import { FormsSection } from './forms-section'
import './content-editor.css'

function FieldInput({ field, value, records = [], selected = [], media = [] }: { field: ContentField; value?: unknown; records?: Awaited<ReturnType<typeof getContentWorkspace>>['records']; selected?: string[]; media?: Awaited<ReturnType<typeof getMediaLibrary>> }) {
  const name = `value_${field.key}`
  if (field.type === 'relationship') return <select name={`relation_${field.key}`} multiple={field.has_many} required={field.required} defaultValue={selected}>
    {!field.required && !field.has_many ? <option value="">None</option> : null}
    {records.filter((record) => record.model_id === field.target_model_id && record.lifecycle_status === 'active').map((record) => <option key={record.id} value={record.id}>{String(record.values.title ?? record.values.name ?? record.values.slug ?? record.id)}</option>)}
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

function RecordForm({ projectId, model, record, components, workspace, media, publicBase }: {
  projectId: string
  model: Awaited<ReturnType<typeof getContentWorkspace>>['models'][number]
  record?: Awaited<ReturnType<typeof getContentWorkspace>>['records'][number]
  components: Awaited<ReturnType<typeof getContentWorkspace>>['models']
  workspace: Awaited<ReturnType<typeof getContentWorkspace>>
  media: Awaited<ReturnType<typeof getMediaLibrary>>
  publicBase?: string
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
      {model.content_role === 'page' ? <PageLayoutEditor components={components} initialLayout={record?.layout ?? []} media={media} publicBase={publicBase} /> : <input type="hidden" name="layout" value="[]" />}
      <div className="action-row"><button type="submit" name="intent" value="draft">Save draft</button><button className="primary" type="submit" name="intent" value="publish">Publish</button></div>
    </form>
  )
}

function ContentModelForm({ projectId, model, models, components }: { projectId: string; model?: ContentModel; models: ContentModel[]; components: ContentModel[] }) {
  const isEdit = Boolean(model)
  const action = model
    ? async (formData: FormData) => { 'use server'; await updateContentModelAction(projectId, model.id, formData) }
    : async (formData: FormData) => { 'use server'; await createContentModelAction(projectId, formData) }
  const role = model?.content_role ?? 'collection'
  const slug = model?.slug ?? ''
  return <form className="content-model-editor" action={action}>
    <div className="form-grid">
      <label>Name<input name="name" required maxLength={160} defaultValue={model?.name ?? ''} placeholder="Articles" /></label>
      {isEdit ? <label>API slug<input value={slug} disabled /><input type="hidden" name="slug" value={slug} /></label> : <label>API slug<input name="slug" required pattern="[a-z][a-z0-9_]*" maxLength={120} placeholder="articles" /></label>}
      {isEdit ? <label>Purpose<input value={role} disabled /><input type="hidden" name="content_role" value={role} /></label> : <label>Purpose<select name="content_role" defaultValue="collection"><option value="collection">Collection — repeatable entries</option><option value="page">Page — fields plus component layout</option><option value="component">Component — reusable page block</option></select></label>}
      <label className="check-label"><input type="checkbox" name="public_read" defaultChecked={model?.public_read ?? false} /> Public when published</label>
      <label className="full">Description<textarea name="description" maxLength={4000} defaultValue={model?.description ?? ''} /></label>
    </div>
    {isEdit ? <p className="immutable-note">API slug and purpose stay immutable so existing site integrations remain stable.</p> : null}
    <fieldset>
      <legend>Components allowed in pages</legend>
      {components.length === 0 ? <div className="muted">No active component schemas exist yet.</div> : <div className="chip-list">{components.map((component) => <label className="chip" key={component.id}><input type="checkbox" name="allowed_component_ids" value={component.id} defaultChecked={model?.allowed_component_ids.includes(component.id) ?? false} /> {component.name}</label>)}</div>}
    </fieldset>
    <ContentModelFieldsEditor initialFields={model?.fields} models={models} />
    <button className="primary" type="submit">{isEdit ? 'Save content type' : 'Create content type'}</button>
  </form>
}

function pageHref(projectId: string, recordPage: number, submissionPage: number) {
  return `/projects/${projectId}/content?record_page=${recordPage}&submission_page=${submissionPage}`
}

export default async function ProjectContentPage({ params, searchParams }: { params: { projectId: string }; searchParams: { submission_page?: string; record_page?: string } }) {
  const parsedSubmissionPage = Number.parseInt(searchParams.submission_page ?? '1', 10)
  const submissionPage = Number.isFinite(parsedSubmissionPage) ? Math.max(1, parsedSubmissionPage) : 1
  const parsedRecordPage = Number.parseInt(searchParams.record_page ?? '1', 10)
  const recordPage = Number.isFinite(parsedRecordPage) ? Math.max(1, parsedRecordPage) : 1
  const [workspace, media, forms] = await Promise.all([getContentWorkspace(params.projectId, recordPage), getMediaLibrary(params.projectId), getFormsWorkspace(params.projectId, submissionPage)])
  const componentModels = workspace.models.filter((model) => model.content_role === 'component' && model.status === 'active')
  const publicBase = process.env.ARGUS_CONTENT_PUBLIC_URL?.replace(/\/$/, '')
  const publishedRecords = workspace.records.filter((record) => record.editorial_status === 'published' && record.lifecycle_status === 'active').length
  const recordPagination = workspace.pagination.records

  return (
    <main className="detail-page">
      <div className="page-header">
        <div>
          <Link className="panel-link" href={`/projects/${params.projectId}`}>← Project</Link>
          <div style={{ marginTop: 10 }}><span className="eyebrow">Content workspace</span><h1>Content</h1><p>Model project content, compose pages visually, manage media and forms, then draft or publish through one project-owned CMS.</p></div>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-card"><div className="stat-label">Content types</div><div className="stat-value">{workspace.models.length}</div><div className="stat-meta">Schemas in this project</div></div>
        <div className="stat-card"><div className="stat-label">Records</div><div className="stat-value">{recordPagination.total_docs}</div><div className="stat-meta">{publishedRecords} published on this page</div></div>
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
                  <label className="check-label"><input name="public_read" type="checkbox" value="true" /> Allow public delivery</label>
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
                          <label className="check-label"><input name="public_read" type="checkbox" defaultChecked={asset.public_read} /> Allow public delivery</label>
                          <button type="submit">Save media details</button>
                        </form>
                        <form action={async (formData) => { 'use server'; await deleteMediaAction(params.projectId, asset.id, formData) }}>
                          <label className="check-label"><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion of original and variants</label>
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
        <div className="detail-card-header"><div><h2>Content types &amp; records</h2><p>Create stable schemas, edit their fields over time, and manage the complete draft/publish/archive lifecycle.</p></div><span className="badge">{workspace.models.length} types</span></div>
        <div className="detail-card-body">
          <details className="create-drawer">
            <summary className="button">+ New content type</summary>
            <div className="drawer-content"><ContentModelForm projectId={params.projectId} models={workspace.models} components={componentModels} /></div>
          </details>

          {workspace.models.length === 0 ? <div className="empty-state"><strong>No content types</strong>Create a schema for pages, collections or reusable components.</div> : (
            <div className="resource-list">
              {workspace.models.map((model) => {
                const records = workspace.records.filter((record) => record.model_id === model.id)
                const allowedComponents = componentModels.filter((component) => model.allowed_component_ids.includes(component.id))
                return (
                  <article className="resource-card" key={model.id}>
                    <div className="resource-card-head">
                      <div><h3>{model.name}</h3><div className="resource-meta">{model.description || 'No description'} · <code>{model.slug}</code> · schema v{model.schema_version}</div></div>
                      <div className="action-row"><span className="badge info">{model.content_role}</span><span className={`badge ${model.status === 'active' ? 'success' : 'warning'}`}>{model.status}</span><span className={`badge ${model.public_read ? 'success' : ''}`}>{model.public_read ? 'Public published records' : 'Private'}</span><span className="badge">{records.length} on page</span></div>
                    </div>
                    <div className="content-lifecycle">
                      <details className="resource-editor"><summary className="button small">Edit content type</summary><div className="resource-editor-body"><ContentModelForm projectId={params.projectId} model={model} models={workspace.models} components={componentModels.filter((component) => component.id !== model.id)} /></div></details>
                      <form action={async () => { 'use server'; await setContentModelStatusAction(params.projectId, model.id, model.status === 'active' ? 'archived' : 'active') }}><button className="small" type="submit">{model.status === 'active' ? 'Archive type' : 'Restore type'}</button></form>
                    </div>
                    <details className="content-danger-zone"><summary>Danger zone</summary><form action={async (formData) => { 'use server'; await deleteContentModelAction(params.projectId, model.id, formData) }}><p className="muted">Permanent deletion is only allowed when the type has no records.</p><label className="check-label"><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion</label><button className="danger small" type="submit">Delete content type</button></form></details>

                    {model.content_role === 'component' ? <div className="callout" style={{ marginTop: 12 }}>Reusable visual block schema. Allow it on a page type to make it appear in that page's visual editor.</div> : model.status === 'archived' ? <div className="callout" style={{ marginTop: 12 }}>This content type is archived. Restore it before creating or editing records.</div> : (
                      <>
                        <details className="resource-editor" style={{ marginTop: 12 }}>
                          <summary className="button small">+ New record</summary>
                          <div className="resource-editor-body"><RecordForm projectId={params.projectId} model={model} components={allowedComponents} workspace={workspace} media={media} publicBase={publicBase} /></div>
                        </details>
                        {records.length === 0 ? <div className="empty-state"><strong>No records on this page</strong>{recordPagination.total_pages > 1 ? 'This content type may have records on another result page.' : `Create the first ${model.name.toLowerCase()} record.`}</div> : (
                          <div className="resource-list" style={{ marginTop: 12 }}>
                            {records.map((record) => {
                              const label = String(record.values.title ?? record.values.name ?? record.values.slug ?? record.id)
                              return (
                                <article className="resource-card" key={record.id}>
                                  <div className="resource-card-head"><div><h4>{label}</h4><div className="resource-meta">{record.published_at ? `Published ${new Date(record.published_at).toLocaleString()}` : 'Not published yet'}</div></div><div className="action-row"><span className={`badge ${record.editorial_status === 'published' ? 'success' : 'warning'}`}>{record.editorial_status}</span><span className={`badge ${record.lifecycle_status === 'active' ? '' : 'warning'}`}>{record.lifecycle_status}</span></div></div>
                                  <div className="action-row">
                                    <Link className="button small" href={`/projects/${params.projectId}/content/preview/${record.id}`}>Preview</Link>
                                    {record.lifecycle_status === 'active' ? <details className="resource-editor"><summary className="button small">Edit record</summary><div className="resource-editor-body"><RecordForm projectId={params.projectId} model={model} record={record} components={allowedComponents} workspace={workspace} media={media} publicBase={publicBase} /></div></details> : null}
                                    <form action={async () => { 'use server'; await setContentRecordStatusAction(params.projectId, record.id, record.lifecycle_status === 'active' ? 'archived' : 'active') }}><button className="small" type="submit">{record.lifecycle_status === 'active' ? 'Archive' : 'Restore'}</button></form>
                                    <details className="resource-editor"><summary className="button small danger">Delete</summary><div className="resource-editor-body"><form action={async (formData) => { 'use server'; await deleteContentRecordAction(params.projectId, record.id, formData) }}><label className="check-label"><input type="checkbox" name="confirm_delete" required /> Permanently delete this record</label><button className="danger small" type="submit">Delete record</button></form></div></details>
                                  </div>
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
          {recordPagination.total_pages > 1 ? <div className="content-pagination">
            <span>Records page {recordPagination.page} of {recordPagination.total_pages} · {recordPagination.total_docs} total</span>
            <div className="action-row">
              {recordPagination.has_prev_page ? <Link className="button small" href={pageHref(params.projectId, recordPagination.page - 1, submissionPage)}>← Previous</Link> : null}
              {recordPagination.has_next_page ? <Link className="button small" href={pageHref(params.projectId, recordPagination.page + 1, submissionPage)}>Next →</Link> : null}
            </div>
          </div> : null}
        </div>
      </section>
    </main>
  )
}
