'use client'

import { useState } from 'react'

import type { ContentField, ContentModel } from '../../../../lib/content-api'

type Props = {
  initialFields?: ContentField[]
  models: ContentModel[]
}

const emptyField = (): ContentField => ({
  key: '',
  label: '',
  type: 'text',
  required: false,
  target_model_id: null,
  has_many: false,
})

export function ContentModelFieldsEditor({ initialFields, models }: Props) {
  const [fields, setFields] = useState<ContentField[]>(initialFields?.length ? initialFields : [{ ...emptyField(), key: 'title', label: 'Title', required: true }])

  const update = (index: number, patch: Partial<ContentField>) => {
    setFields((current) => current.map((field, fieldIndex) => fieldIndex === index ? { ...field, ...patch } : field))
  }
  const move = (index: number, offset: number) => {
    setFields((current) => {
      const destination = index + offset
      if (destination < 0 || destination >= current.length) return current
      const next = [...current]
      ;[next[index], next[destination]] = [next[destination], next[index]]
      return next
    })
  }

  return (
    <fieldset className="schema-builder">
      <div className="schema-builder-heading">
        <div><legend>Fields</legend><p>Define up to 50 typed fields. Order is preserved in the editor.</p></div>
        <button type="button" className="small" disabled={fields.length >= 50} onClick={() => setFields((current) => [...current, emptyField()])}>+ Add field</button>
      </div>
      <div className="schema-field-list">
        {fields.map((field, index) => (
          <article className="schema-field" key={`${field.key}-${index}`}>
            <div className="schema-field-index">{index + 1}</div>
            <div className="schema-field-grid">
              <label>Label<input name={`field_${index}_label`} value={field.label} maxLength={160} required onChange={(event) => update(index, { label: event.target.value })} /></label>
              <label>Key<input name={`field_${index}_key`} value={field.key} maxLength={120} pattern="[a-z][a-z0-9_]*" required onChange={(event) => update(index, { key: event.target.value.toLowerCase().replace(/[^a-z0-9_]/g, '') })} /></label>
              <label>Type<select name={`field_${index}_type`} value={field.type} onChange={(event) => update(index, { type: event.target.value as ContentField['type'], target_model_id: event.target.value === 'relationship' ? field.target_model_id : null, has_many: ['relationship', 'media'].includes(event.target.value) ? field.has_many : false })}>
                <option value="text">Short text</option><option value="textarea">Long text</option><option value="number">Number</option><option value="boolean">Yes / no</option><option value="date">Date</option><option value="datetime">Date and time</option><option value="json">Structured JSON</option><option value="relationship">Relationship</option><option value="media">Media image</option>
              </select></label>
              {field.type === 'relationship' ? <label>Relationship target<select name={`field_${index}_target_model_id`} value={field.target_model_id ?? ''} required onChange={(event) => update(index, { target_model_id: event.target.value || null })}><option value="">Choose a content type</option>{models.filter((model) => model.content_role !== 'component').map((model) => <option key={model.id} value={model.id}>{model.name}</option>)}</select></label> : <input type="hidden" name={`field_${index}_target_model_id`} value="" />}
              <label className="check-label"><input type="checkbox" name={`field_${index}_required`} checked={field.required} onChange={(event) => update(index, { required: event.target.checked })} /> Required</label>
              {['relationship', 'media'].includes(field.type) ? <label className="check-label"><input type="checkbox" name={`field_${index}_has_many`} checked={field.has_many} onChange={(event) => update(index, { has_many: event.target.checked })} /> Multiple values</label> : <input type="hidden" name={`field_${index}_has_many`} value="" />}
            </div>
            <div className="schema-field-actions">
              <button type="button" className="small" disabled={index === 0} onClick={() => move(index, -1)} aria-label={`Move ${field.label || 'field'} up`}>↑</button>
              <button type="button" className="small" disabled={index === fields.length - 1} onClick={() => move(index, 1)} aria-label={`Move ${field.label || 'field'} down`}>↓</button>
              <button type="button" className="small danger" disabled={fields.length === 1} onClick={() => setFields((current) => current.filter((_, fieldIndex) => fieldIndex !== index))}>Remove</button>
            </div>
          </article>
        ))}
      </div>
    </fieldset>
  )
}
