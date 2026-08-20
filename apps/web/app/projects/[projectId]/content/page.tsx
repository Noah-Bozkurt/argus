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
  if (field.type === 'textarea' || field.type === 'json') {
    return <textarea name={name} required={field.required} defaultValue={field.type === 'json' && value !== undefined ? JSON.stringify(value, null, 2) : String(value ?? '')} />
  }
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
      {model.fields.map((field) => (
        <label key={field.key}>
          {field.label}{field.required ? ' *' : ''}
          <FieldInput field={field} value={record?.values[field.key]} records={workspace.records} media={media}
            selected={record ? workspace.relations.filter((relation) => relation.source_record_id === record.id && relation.field_key === field.key).map((relation) => relation.target_record_id) : []} />
        </label>
      ))}
      {model.content_role === 'page' ? <PageLayoutEditor components={components} initialLayout={record?.layout ?? []} media={media} /> : <input type="hidden" name="layout" value="[]" />}
      <button type="submit" name="intent" value="draft">Save draft</button>
      <button type="submit" name="intent" value="publish">Publish</button>
    </form>
  )
}

export default async function ProjectContentPage({ params, searchParams }: { params: { projectId: string }; searchParams: { submission_page?: string } }) {
  const parsedPage = Number.parseInt(searchParams.submission_page ?? '1', 10)
  const submissionPage = Number.isFinite(parsedPage) ? Math.max(1, parsedPage) : 1
  const [workspace, media, forms] = await Promise.all([getContentWorkspace(params.projectId), getMediaLibrary(params.projectId), getFormsWorkspace(params.projectId, submissionPage)])
  const componentModels = workspace.models.filter((model) => model.content_role === 'component')
  const publicBase = process.env.ARGUS_CONTENT_PUBLIC_URL?.replace(/\/$/, '')
  return (
    <main>
      <p><Link href={`/projects/${params.projectId}`}>← Project</Link></p>
      <h1>Content</h1>
      <p>Create project-owned content types, save drafts, and publish records. Argus handles the Payload storage details.</p>

      <h2>Media library</h2>
      <p>Upload project-owned images up to 10 MiB. Argus creates thumbnail, medium and large variants. Public delivery must be enabled explicitly.</p>
      <form action={async (formData) => { 'use server'; await uploadMediaAction(params.projectId, formData) }}>
        <label>Image<input name="file" type="file" accept="image/jpeg,image/png,image/webp,image/avif" required /></label>
        <label>Alternative text<input name="alt" required maxLength={300} /></label>
        <label>Caption<textarea name="caption" maxLength={2000} /></label>
        <label><input name="public_read" type="checkbox" value="true" /> Allow public delivery</label>
        <button type="submit">Upload image</button>
      </form>
      {media.length === 0 ? <p>No media uploaded.</p> : <ul>{media.map((asset) => <li key={asset.id}>
        <strong>{asset.alt}</strong> — {asset.filename} — {asset.width ?? '?'}×{asset.height ?? '?'} — {Math.ceil(asset.filesize / 1024)} KiB — {asset.public_read ? 'public' : 'private'}
        {asset.public_read && asset.url && publicBase ? <> — <a href={`${publicBase}${asset.url}`} target="_blank" rel="noreferrer">View image</a></> : null}
        <form action={async (formData) => { 'use server'; await updateMediaAction(params.projectId, asset.id, formData) }}>
          <label>Alternative text<input name="alt" required maxLength={300} defaultValue={asset.alt} /></label>
          <label>Caption<textarea name="caption" maxLength={2000} defaultValue={asset.caption} /></label>
          <label><input name="public_read" type="checkbox" defaultChecked={asset.public_read} /> Allow public delivery</label>
          <button type="submit">Save media details</button>
        </form>
        <form action={async (formData) => { 'use server'; await deleteMediaAction(params.projectId, asset.id, formData) }}>
          <label><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion of the original and all variants</label>
          <button type="submit">Delete media</button>
        </form>
      </li>)}</ul>}

      <FormsSection projectId={params.projectId} workspace={forms} publicBase={publicBase} />

      <h2>New content type</h2>
      <form action={async (formData) => { 'use server'; await createContentModelAction(params.projectId, formData) }}>
        <label>Name<input name="name" required maxLength={160} placeholder="Articles" /></label>
        <label>API slug<input name="slug" required pattern="[a-z][a-z0-9_]*" maxLength={120} placeholder="articles" /></label>
        <label>Description<textarea name="description" maxLength={4000} /></label>
        <label>Purpose<select name="content_role" defaultValue="collection">
          <option value="collection">Collection — repeatable standalone entries</option>
          <option value="page">Page — fields plus a component layout</option>
          <option value="component">Component — reusable page block schema</option>
        </select></label>
        <label><input type="checkbox" name="public_read" /> Allow published records to be read publicly</label>
        <fieldset>
          <legend>Components allowed in pages</legend>
          {componentModels.length === 0 ? <p>No component schemas exist yet.</p> : componentModels.map((component) => (
            <label key={component.id}><input type="checkbox" name="allowed_component_ids" value={component.id} /> {component.name}</label>
          ))}
        </fieldset>
        <fieldset>
          <legend>Fields</legend>
          {[0, 1, 2, 3, 4].map((index) => (
            <div key={index}>
              <input name={`field_${index}_label`} placeholder={index === 0 ? 'Title' : 'Field label'} required={index === 0} />
              <input name={`field_${index}_key`} placeholder={index === 0 ? 'title' : 'field_key'} pattern="[a-z][a-z0-9_]*" required={index === 0} />
              <select name={`field_${index}_type`} defaultValue={index === 1 ? 'textarea' : 'text'}>
                <option value="text">Short text</option><option value="textarea">Long text</option><option value="number">Number</option>
                <option value="boolean">Yes / no</option><option value="date">Date</option><option value="datetime">Date and time</option><option value="json">Structured JSON</option>
                <option value="relationship">Relationship</option>
                <option value="media">Media image</option>
              </select>
              <label><input type="checkbox" name={`field_${index}_required`} defaultChecked={index === 0} /> Required</label>
              <label>Relationship target<select name={`field_${index}_target_model_id`} defaultValue=""><option value="">Not applicable</option>{workspace.models.filter((candidate) => candidate.content_role !== 'component').map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.name}</option>)}</select></label>
              <label><input type="checkbox" name={`field_${index}_has_many`} /> Allow multiple selections (relationships or media)</label>
            </div>
          ))}
        </fieldset>
        <button type="submit">Create content type</button>
      </form>

      {workspace.models.length === 0 ? <p>No content types yet.</p> : workspace.models.map((model) => {
        const records = workspace.records.filter((record) => record.model_id === model.id)
        return (
          <section key={model.id}>
            <h2>{model.name}</h2>
            <p>{model.description || 'No description.'} — {model.content_role} — slug <code>{model.slug}</code> — schema v{model.schema_version} — {model.public_read ? 'public when published' : 'private'}</p>
            {model.content_role === 'component' ? <p>This schema is available as a block in page types that allow it.</p> : <>
            <h3>New record</h3>
            <RecordForm projectId={params.projectId} model={model} components={componentModels.filter((component) => model.allowed_component_ids.includes(component.id))} workspace={workspace} media={media} />
            <h3>Existing records</h3>
            {records.length === 0 ? <p>No records yet.</p> : records.map((record) => (
              <article key={record.id}>
                <p><strong>{record.editorial_status === 'published' ? 'Published' : 'Draft'}</strong>{record.published_at ? ` — ${new Date(record.published_at).toLocaleString()}` : ''}</p>
                <p><Link href={`/projects/${params.projectId}/content/preview/${record.id}`}>Preview</Link></p>
                <RecordForm projectId={params.projectId} model={model} record={record} components={componentModels.filter((component) => model.allowed_component_ids.includes(component.id))} workspace={workspace} media={media} />
              </article>
            ))}</>}
          </section>
        )
      })}
    </main>
  )
}
