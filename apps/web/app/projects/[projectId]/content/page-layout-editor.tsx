'use client'

import { useMemo, useState } from 'react'

import type { ContentBlock, ContentField, ContentModel, MediaAsset } from '../../../../lib/content-api'

function inputValue(value: unknown): string | number | readonly string[] | undefined {
  if (value === undefined || value === null) return ''
  return typeof value === 'string' || typeof value === 'number' ? value : JSON.stringify(value, null, 2)
}

function BlockField({ block, field, media, onChange }: { block: ContentBlock; field: ContentField; media: MediaAsset[]; onChange: (value: unknown) => void }) {
  const value = block.values[field.key]
  const label = <>{field.label}{field.required ? ' *' : ''}</>
  if (field.type === 'boolean') return <label className="check-label">{label}<input type="checkbox" checked={value === true} onChange={(event) => onChange(event.target.checked)} /></label>
  if (field.type === 'media') {
    const selected = Array.isArray(value) ? value.map(String) : value ? [String(value)] : []
    return <label>{label}<select multiple={field.has_many} required={field.required} value={selected} onChange={(event) => {
      const values = Array.from(event.currentTarget.selectedOptions, (option) => option.value).filter(Boolean)
      onChange(field.has_many ? values : (values[0] ?? ''))
    }}><option value="">None</option>{media.map((asset) => <option key={asset.id} value={asset.id}>{asset.alt} — {asset.filename}</option>)}</select></label>
  }
  if (field.type === 'textarea' || field.type === 'json') {
    return <label>{label}<textarea required={field.required} value={inputValue(value)} onChange={(event) => {
      if (field.type !== 'json') return onChange(event.target.value)
      try { onChange(JSON.parse(event.target.value)) } catch { onChange(event.target.value) }
    }} /></label>
  }
  const type = field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : field.type === 'datetime' ? 'datetime-local' : 'text'
  return <label>{label}<input type={type} required={field.required} value={inputValue(value)} onChange={(event) => onChange(field.type === 'number' ? Number(event.target.value) : event.target.value)} /></label>
}

function firstText(block: ContentBlock, keys: string[]): string {
  for (const key of keys) {
    const value = block.values[key]
    if (typeof value === 'string' && value.trim()) return value
  }
  return ''
}

function mediaForBlock(block: ContentBlock, component: ContentModel, media: MediaAsset[]) {
  for (const field of component.fields.filter((field) => field.type === 'media')) {
    const value = block.values[field.key]
    const id = Array.isArray(value) ? String(value[0] ?? '') : String(value ?? '')
    const asset = media.find((candidate) => candidate.id === id)
    if (asset) return asset
  }
  return null
}

function BlockPreview({ block, component, media, publicBase }: { block: ContentBlock; component: ContentModel; media: MediaAsset[]; publicBase?: string }) {
  const heading = firstText(block, ['heading', 'title', 'name', 'label']) || component.name
  const body = firstText(block, ['body', 'description', 'text', 'copy', 'subtitle'])
  const asset = mediaForBlock(block, component, media)
  const imageUrl = asset?.url && publicBase ? `${publicBase}${asset.url}` : null
  const populated = Object.values(block.values).filter((value) => value !== '' && value !== null && value !== undefined).length

  return <div className="visual-block-preview">
    {imageUrl ? <div className="visual-block-image" style={{ backgroundImage: `url(${JSON.stringify(imageUrl).slice(1, -1)})` }} /> : null}
    <div className="visual-block-copy">
      <span className="eyebrow">{component.name}</span>
      <h3>{heading}</h3>
      {body ? <p>{body}</p> : <p className="muted">Select this block to edit its content.</p>}
      <div className="visual-block-meta">{populated}/{component.fields.length} fields filled</div>
    </div>
  </div>
}

export function PageLayoutEditor({ components, initialLayout, media, publicBase }: { components: ContentModel[]; initialLayout: ContentBlock[]; media: MediaAsset[]; publicBase?: string }) {
  const [blocks, setBlocks] = useState(initialLayout)
  const [selectedId, setSelectedId] = useState<string | null>(initialLayout[0]?.id ?? null)
  const [device, setDevice] = useState<'desktop' | 'tablet' | 'mobile'>('desktop')
  const [dragIndex, setDragIndex] = useState<number | null>(null)
  const selectedIndex = blocks.findIndex((block) => block.id === selectedId)
  const selected = selectedIndex >= 0 ? blocks[selectedIndex] : null
  const selectedComponent = selected ? components.find((component) => component.slug === selected.component) : null
  const componentMap = useMemo(() => new Map(components.map((component) => [component.slug, component])), [components])

  const update = (index: number, block: ContentBlock) => setBlocks((current) => current.map((item, itemIndex) => itemIndex === index ? block : item))
  const add = (component: ContentModel) => {
    const block = { id: crypto.randomUUID(), component: component.slug, values: {} }
    setBlocks((current) => [...current, block])
    setSelectedId(block.id)
  }
  const remove = (id: string) => {
    setBlocks((current) => current.filter((block) => block.id !== id))
    setSelectedId((current) => current === id ? null : current)
  }
  const duplicate = (block: ContentBlock) => {
    const copy = { ...block, id: crypto.randomUUID(), values: structuredClone(block.values) }
    setBlocks((current) => {
      const index = current.findIndex((item) => item.id === block.id)
      const next = [...current]
      next.splice(index + 1, 0, copy)
      return next
    })
    setSelectedId(copy.id)
  }
  const dropAt = (destination: number) => {
    if (dragIndex === null || dragIndex === destination) return setDragIndex(null)
    setBlocks((current) => {
      const next = [...current]
      const [moved] = next.splice(dragIndex, 1)
      next.splice(destination, 0, moved)
      return next
    })
    setDragIndex(null)
  }

  return <fieldset className="visual-editor-fieldset">
    <legend>Visual page editor</legend>
    <input type="hidden" name="layout" value={JSON.stringify(blocks)} />
    <div className="visual-editor">
      <aside className="visual-editor-palette">
        <div className="visual-pane-heading"><strong>Blocks</strong><span>{components.length}</span></div>
        <p>Build the page from project component schemas.</p>
        <div className="visual-palette-list">
          {components.map((component) => <button key={component.id} type="button" className="visual-palette-item" onClick={() => add(component)}><span>+</span><div><strong>{component.name}</strong><small>{component.description || `${component.fields.length} fields`}</small></div></button>)}
          {components.length === 0 ? <div className="muted">Allow one or more component schemas on this page type first.</div> : null}
        </div>
      </aside>

      <section className="visual-editor-stage">
        <div className="visual-editor-toolbar">
          <div><strong>Canvas</strong><span>{blocks.length} blocks</span></div>
          <div className="visual-device-switcher" aria-label="Preview width">
            {(['desktop', 'tablet', 'mobile'] as const).map((option) => <button key={option} type="button" className={device === option ? 'active' : ''} onClick={() => setDevice(option)}>{option === 'desktop' ? 'Desktop' : option === 'tablet' ? 'Tablet' : 'Mobile'}</button>)}
          </div>
        </div>
        <div className={`visual-canvas visual-canvas-${device}`}>
          {blocks.length === 0 ? <button type="button" className="visual-empty-canvas" onClick={() => components[0] && add(components[0])}><strong>Start composing this page</strong><span>{components.length ? 'Add a block from the left, or click here to add the first one.' : 'Create and allow a component schema before composing the page.'}</span></button> : blocks.map((block, index) => {
            const component = componentMap.get(block.component)
            if (!component) return <div key={block.id} className="visual-block unavailable">Unavailable component: {block.component}</div>
            return <div key={block.id} className={`visual-block ${selectedId === block.id ? 'selected' : ''}`} draggable onDragStart={() => setDragIndex(index)} onDragOver={(event) => event.preventDefault()} onDrop={() => dropAt(index)} onClick={() => setSelectedId(block.id)}>
              <div className="visual-block-handle" title="Drag to reorder">⋮⋮</div>
              <BlockPreview block={block} component={component} media={media} publicBase={publicBase} />
              <div className="visual-block-actions"><button type="button" className="small" onClick={(event) => { event.stopPropagation(); duplicate(block) }}>Duplicate</button><button type="button" className="small danger" onClick={(event) => { event.stopPropagation(); remove(block.id) }}>Remove</button></div>
            </div>
          })}
        </div>
      </section>

      <aside className="visual-editor-inspector">
        <div className="visual-pane-heading"><strong>Inspector</strong>{selectedComponent ? <span>{selectedComponent.name}</span> : null}</div>
        {!selected || !selectedComponent ? <p>Select a block on the canvas to edit it.</p> : <div className="visual-inspector-fields">
          <div className="visual-inspector-title"><span className="badge info">{selectedComponent.name}</span><small>{selected.component}</small></div>
          {selectedComponent.fields.map((field) => <BlockField key={field.key} block={selected} field={field} media={media} onChange={(value) => update(selectedIndex, { ...selected, values: { ...selected.values, [field.key]: value } })} />)}
        </div>}
      </aside>
    </div>
  </fieldset>
}
