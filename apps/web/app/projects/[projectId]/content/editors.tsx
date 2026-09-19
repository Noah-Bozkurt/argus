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
  if (field.type === 'textarea' || field.type === 'json') return <textarea data-argus-type={field.type} name={name} required={field.required} defaultValue={field.type === 'json' && value !== undefined ? JSON.stringify(value, null, 2) : String(value ?? '')} />
  if (field.type === 'boolean') return <input name={name} type="checkbox" defaultChecked={value === true} />
  const type = field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : field.type === 'datetime' ? 'datetime-local' : 'text'
  return <input name={name} type={type} required={field.required} defaultValue={String(value ?? '')} />
}

export async function RecordForm({ projectId, model, record, components, workspace, media, publicBase }: {
  projectId: string
  model: Awaited<ReturnType<typeof getContentWorkspace>>['models'][number]
  record?: Awaited<ReturnType<typeof getContentWorkspace>>['records'][number]
  components: Awaited<ReturnType<typeof getContentWorkspace>>['models']
  workspace: Awaited<ReturnType<typeof getContentWorkspace>>
  media: Awaited<ReturnType<typeof getMediaLibrary>>
  publicBase?: string
}) {
  const targets = [...new Set(model.fields.filter(field => field.type === 'relationship' && field.target_model_id).map(field => field.target_model_id!))]
  const related = await Promise.all(targets.map(async target => {
    const records = [] as typeof workspace.records
    let page = 1
    while (true) {
      const result = await getContentWorkspace(projectId, page, target)
      records.push(...result.records)
      if (!result.pagination.records.has_next_page) return records
      page++
    }
  }))
  const selectableRecords = [...workspace.records, ...related.flat()]
  return (
    <form action={async (formData) => { 'use server'; await saveContentRecordAction(projectId, model.fields, formData) }}>
      <input type="hidden" name="model_id" value={model.id} />
      <input type="hidden" name="record_id" value={record?.id ?? ''} />
      <div className="form-grid">
        {model.fields.map((field) => (
          <label className={field.type === 'textarea' || field.type === 'json' ? 'full' : undefined} key={field.key}>
            {field.label}{field.required ? ' *' : ''}
            <FieldInput field={field} value={record?.values[field.key]} records={selectableRecords} media={media} selected={record ? workspace.relations.filter((relation) => relation.source_record_id === record.id && relation.field_key === field.key).map((relation) => relation.target_record_id) : []} />
          </label>
        ))}
      </div>
      {model.content_role === 'page' ? <PageLayoutEditor projectId={projectId} recordId={record?.id} components={components} initialLayout={record?.layout ?? []} media={media} publicBase={publicBase} /> : <input type="hidden" name="layout" value="[]" />}
      <div className="action-row"><button type="submit" name="intent" value="draft">Save draft</button><Link className="button primary" href={`/projects/${projectId}/content/publish`}>Review publication</Link></div>
    </form>
  )
}

export function ContentModelForm({ projectId, model, models, components }: { projectId: string; model?: ContentModel; models: ContentModel[]; components: ContentModel[] }) {
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

