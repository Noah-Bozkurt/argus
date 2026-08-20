'use client'

import { useState } from 'react'

type Row = { id: string; type: string }

export function FormFieldsEditor() {
  const [rows, setRows] = useState<Row[]>([{ id: 'initial', type: 'email' }])
  return <fieldset>
    <legend>Fields (up to 30)</legend>
    {rows.map((row, index) => <div key={row.id}>
      <input name={`form_field_${index}_label`} placeholder={index === 0 ? 'Email' : 'Field label'} required />
      <input name={`form_field_${index}_key`} placeholder={index === 0 ? 'email' : 'field_key'} pattern="[a-z][a-z0-9_]*" required />
      <select name={`form_field_${index}_type`} value={row.type} onChange={(event) => setRows((current) => current.map((candidate) => candidate.id === row.id ? { ...candidate, type: event.target.value } : candidate))}>
        <option value="text">Short text</option><option value="email">Email</option><option value="textarea">Long text</option>
        <option value="number">Number</option><option value="boolean">Checkbox / consent</option><option value="select">Select</option>
      </select>
      {row.type === 'select' ? <input name={`form_field_${index}_options`} placeholder="Choices, comma separated" required /> : null}
      <label><input type="checkbox" name={`form_field_${index}_required`} defaultChecked={index === 0} /> Required</label>
      {rows.length > 1 ? <button type="button" onClick={() => setRows((current) => current.filter((candidate) => candidate.id !== row.id))}>Remove field</button> : null}
    </div>)}
    <button type="button" disabled={rows.length >= 30} onClick={() => setRows((current) => [...current, { id: crypto.randomUUID(), type: 'text' }])}>Add field</button>
  </fieldset>
}
