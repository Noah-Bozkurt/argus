'use client'
import { useEffect, useRef, useState } from 'react'
import type { ContentBlock } from '../../../../lib/content-api'
export function SitePreview({ projectId, recordId, blocks, onSelect }: { projectId: string; recordId: string; blocks: ContentBlock[]; onSelect: (id: string) => void }) {
 const frame = useRef<HTMLIFrameElement>(null)
 const [session, setSession] = useState<{ url: string; token: string } | null>(null)
 const [error, setError] = useState('')
 const [ready, setReady] = useState(false)
 useEffect(() => {
  const controller = new AbortController()
  async function connect() {
   try {
    const response = await fetch(`/api/projects/${projectId}/site`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action: 'preview', recordId }), signal: controller.signal })
    const result = await response.json()
    if (!response.ok) { setError(result.code === 'SITE_NOT_CONNECTED' ? 'Connect a preview site in Site connection to see your website here.' : 'Preview is unavailable. Check your site connection.'); return }
    setSession(result); setError('')
   } catch { if (!controller.signal.aborted) setError('Unable to connect to preview.') }
  }
  void connect(); const timer = setInterval(connect, 240000)
  return () => { controller.abort(); clearInterval(timer) }
 }, [projectId, recordId])
 useEffect(() => {
  if (!session) return
  const receive = (event: MessageEvent) => {
   if (event.source !== frame.current?.contentWindow || event.origin !== new URL(session.url).origin) return
   if (event.data?.type === 'argus:ready') setReady(true)
   if (event.data?.type === 'argus:select' && blocks.some(block => block.id === event.data.id)) onSelect(event.data.id)
   if (event.data?.type === 'argus:error') setError('The website could not render this preview. Check its component mappings.')
  }
  window.addEventListener('message', receive)
  return () => window.removeEventListener('message', receive)
 }, [session, blocks, onSelect])
 useEffect(() => {
  if (!session || !ready) return
  const form = frame.current?.closest('form')
  let timer: ReturnType<typeof setTimeout>
  const render = () => {
   clearTimeout(timer)
   timer = setTimeout(() => {
    const values: Record<string, unknown> = {}
    if (form) for (const element of Array.from(form.elements)) {
     if (!(element instanceof HTMLInputElement || element instanceof HTMLSelectElement || element instanceof HTMLTextAreaElement) || !element.name.startsWith('value_')) continue
     const key = element.name.slice(6)
     if (element instanceof HTMLInputElement && element.type === 'checkbox') values[key] = element.checked
     else if (element instanceof HTMLInputElement && element.type === 'number') values[key] = element.value === '' ? null : element.valueAsNumber
     else if (element instanceof HTMLSelectElement && element.multiple) values[key] = Array.from(element.selectedOptions, option => option.value)
     else if (element.dataset.argusType === 'json') {
      try { values[key] = element.value.trim() ? JSON.parse(element.value) : null } catch { return }
     } else values[key] = element.value
    }
    frame.current?.contentWindow?.postMessage({ type: 'argus:render', token: session.token, layout: blocks, values }, new URL(session.url).origin)
   }, 300)
  }
  render()
  form?.addEventListener('input', render)
  form?.addEventListener('change', render)
  return () => {
   clearTimeout(timer)
   form?.removeEventListener('input', render)
   form?.removeEventListener('change', render)
  }
 }, [blocks, session, ready])
 return <section><p role="status">{error || (ready ? 'Live draft preview' : 'Connecting to website preview…')}</p>{session && <iframe ref={frame} title="Website draft preview" src={session.url} sandbox="allow-scripts allow-same-origin" referrerPolicy="no-referrer" style={{ width: '100%', height: 640, border: 0 }} />}</section>
}
