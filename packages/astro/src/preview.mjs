/** Install only on the isolated preview site. operatorOrigin is explicit configuration. */
export function connectPreview({ operatorOrigin, renderURL = location.href }) {
  const origin = new URL(operatorOrigin).origin
  let controller
  const receive = async event => {
    if (event.origin !== origin || event.source !== parent || event.data?.type !== 'argus:render') return
    const { token, layout, values } = event.data
    if (typeof token !== 'string' || !Array.isArray(layout)) return
    controller?.abort(); controller = new AbortController()
    try {
      const response = await fetch(renderURL, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' }, body: JSON.stringify({ layout, values }), signal: controller.signal, cache: 'no-store' })
      if (!response.ok) throw new Error('Preview failed')
      const html = new DOMParser().parseFromString(await response.text(), 'text/html')
      const next = html.querySelector('[data-argus-canvas]')
      const current = document.querySelector('[data-argus-canvas]')
      if (!next || !current) throw new Error('Preview canvas missing')
      current.replaceWith(next)
    } catch (error) {
      if (error.name !== 'AbortError') parent.postMessage({ type: 'argus:error' }, origin)
    }
  }
  const select = event => {
    const block = event.target.closest('[data-argus-block]')
    if (block) { event.preventDefault(); parent.postMessage({ type: 'argus:select', id: block.dataset.argusBlock }, origin) }
    else if (event.target.closest('a, form')) event.preventDefault()
  }
  const preventSubmit = event => event.preventDefault()
  document.addEventListener('submit', preventSubmit)
  window.addEventListener('message', receive)
  document.addEventListener('click', select)
  parent.postMessage({ type: 'argus:ready' }, origin)
  return () => { controller?.abort(); window.removeEventListener('message', receive); document.removeEventListener('click', select); document.removeEventListener('submit', preventSubmit) }
}
