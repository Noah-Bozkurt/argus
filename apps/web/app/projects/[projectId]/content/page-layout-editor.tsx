'use client'

import { useState } from 'react'

import type { ContentBlock, ContentField, ContentModel } from '../../../../lib/content-api'

function inputValue(value: unknown): string | number | readonly string[] | undefined {
  if (value === undefined || value === null) return ''
  return typeof value === 'string' || typeof value === 'number' ? value : JSON.stringify(value, null, 2)
}

function BlockField({ block, field, onChange }: { block: ContentBlock; field: ContentField; onChange: (value: unknown) => void }) {
  const value = block.values[field.key]
  const label = <>{field.label}{field.required ? ' *' : ''}</>
  if (field.type === 'boolean') return <label>{label}<input type="checkbox" checked={value === true} onChange={(event) => onChange(event.target.checked)} /></label>
  if (field.type === 'textarea' || field.type === 'json') {
    return <label>{label}<textarea required={field.required} value={inputValue(value)} onChange={(event) => {
      if (field.type !== 'json') return onChange(event.target.value)
      try { onChange(JSON.parse(event.target.value)) } catch { onChange(event.target.value) }
    }} /></label>
  }
  const type = field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : field.type === 'datetime' ? 'datetime-local' : 'text'
  return <label>{label}<input type={type} required={field.required} value={inputValue(value)} onChange={(event) => onChange(field.type === 'number' ? Number(event.target.value) : event.target.value)} /></label>
}

export function PageLayoutEditor({ components, initialLayout }: { components: ContentModel[]; initialLayout: ContentBlock[] }) {
  const [blocks, setBlocks] = useState(initialLayout)
  const update = (index: number, block: ContentBlock) => setBlocks((current) => current.map((item, itemIndex) => itemIndex === index ? block : item))
  const move = (index: number, offset: number) => setBlocks((current) => {
    const destination = index + offset
    if (destination < 0 || destination >= current.length) return current
    const next = [...current]
    ;[next[index], next[destination]] = [next[destination], next[index]]
    return next
  })

  return <fieldset>
    <legend>Page layout</legend>
    <input type="hidden" name="layout" value={JSON.stringify(blocks)} />
    {blocks.map((block, index) => {
      const component = components.find((candidate) => candidate.slug === block.component)
      if (!component) return <p key={block.id}>Unavailable component: {block.component}</p>
      return <section key={block.id}>
        <h4>{component.name}</h4>
        {component.fields.map((field) => <BlockField key={field.key} block={block} field={field} onChange={(value) => update(index, { ...block, values: { ...block.values, [field.key]: value } })} />)}
        <button type="button" onClick={() => move(index, -1)} disabled={index === 0}>Move up</button>
        <button type="button" onClick={() => move(index, 1)} disabled={index === blocks.length - 1}>Move down</button>
        <button type="button" onClick={() => setBlocks((current) => current.filter((_, itemIndex) => itemIndex !== index))}>Remove</button>
      </section>
    })}
    {components.length === 0 ? <p>Add a component schema to this page type before composing a layout.</p> : components.map((component) => (
      <button key={component.id} type="button" onClick={() => setBlocks((current) => [...current, { id: crypto.randomUUID(), component: component.slug, values: {} }])}>Add {component.name}</button>
    ))}
  </fieldset>
}
