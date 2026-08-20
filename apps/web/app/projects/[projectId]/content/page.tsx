import Link from 'next/link'

import { getContentWorkspace, type ContentField } from '../../../../lib/content-api'
import { createContentModelAction, saveContentRecordAction } from './actions'

function FieldInput({ field, value }: { field: ContentField; value?: unknown }) {
  const name = `value_${field.key}`
  if (field.type === 'textarea' || field.type === 'json') {
    return <textarea name={name} required={field.required} defaultValue={field.type === 'json' && value !== undefined ? JSON.stringify(value, null, 2) : String(value ?? '')} />
  }
  if (field.type === 'boolean') return <input name={name} type="checkbox" defaultChecked={value === true} />
  const type = field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : field.type === 'datetime' ? 'datetime-local' : 'text'
  return <input name={name} type={type} required={field.required} defaultValue={String(value ?? '')} />
}

function RecordForm({ projectId, model, record }: {
  projectId: string
  model: Awaited<ReturnType<typeof getContentWorkspace>>['models'][number]
  record?: Awaited<ReturnType<typeof getContentWorkspace>>['records'][number]
}) {
  return (
    <form action={async (formData) => { 'use server'; await saveContentRecordAction(projectId, model.fields, formData) }}>
      <input type="hidden" name="model_id" value={model.id} />
      <input type="hidden" name="record_id" value={record?.id ?? ''} />
      {model.fields.map((field) => (
        <label key={field.key}>
          {field.label}{field.required ? ' *' : ''}
          <FieldInput field={field} value={record?.values[field.key]} />
        </label>
      ))}
      <button type="submit" name="intent" value="draft">Save draft</button>
      <button type="submit" name="intent" value="publish">Publish</button>
    </form>
  )
}

export default async function ProjectContentPage({ params }: { params: { projectId: string } }) {
  const workspace = await getContentWorkspace(params.projectId)
  return (
    <main>
      <p><Link href={`/projects/${params.projectId}`}>← Project</Link></p>
      <h1>Content</h1>
      <p>Create project-owned content types, save drafts, and publish records. Argus handles the Payload storage details.</p>

      <h2>New content type</h2>
      <form action={async (formData) => { 'use server'; await createContentModelAction(params.projectId, formData) }}>
        <label>Name<input name="name" required maxLength={160} placeholder="Articles" /></label>
        <label>API slug<input name="slug" required pattern="[a-z][a-z0-9_]*" maxLength={120} placeholder="articles" /></label>
        <label>Description<textarea name="description" maxLength={4000} /></label>
        <label><input type="checkbox" name="public_read" /> Allow published records to be read publicly</label>
        <fieldset>
          <legend>Fields</legend>
          {[0, 1, 2, 3, 4].map((index) => (
            <div key={index}>
              <input name={`field_${index}_label`} placeholder={index === 0 ? 'Title' : 'Field label'} required={index === 0} />
              <input name={`field_${index}_key`} placeholder={index === 0 ? 'title' : 'field_key'} pattern="[a-z][a-z0-9_]*" required={index === 0} />
              <select name={`field_${index}_type`} defaultValue={index === 1 ? 'textarea' : 'text'}>
                <option value="text">Short text</option><option value="textarea">Long text</option><option value="number">Number</option>
                <option value="boolean">Yes / no</option><option value="date">Date</option><option value="datetime">Date and time</option><option value="json">Structured JSON</option>
              </select>
              <label><input type="checkbox" name={`field_${index}_required`} defaultChecked={index === 0} /> Required</label>
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
            <p>{model.description || 'No description.'} — slug <code>{model.slug}</code> — schema v{model.schema_version} — {model.public_read ? 'public when published' : 'private'}</p>
            <h3>New record</h3>
            <RecordForm projectId={params.projectId} model={model} />
            <h3>Existing records</h3>
            {records.length === 0 ? <p>No records yet.</p> : records.map((record) => (
              <article key={record.id}>
                <p><strong>{record.editorial_status === 'published' ? 'Published' : 'Draft'}</strong>{record.published_at ? ` — ${new Date(record.published_at).toLocaleString()}` : ''}</p>
                <p><Link href={`/projects/${params.projectId}/content/preview/${record.id}`}>Preview</Link></p>
                <RecordForm projectId={params.projectId} model={model} record={record} />
              </article>
            ))}
          </section>
        )
      })}
    </main>
  )
}
