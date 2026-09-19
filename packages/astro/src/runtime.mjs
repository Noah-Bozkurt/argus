/** Validate a signed draft grant server-side. Never use an internal Argus token here. */
export async function loadPreview({ contentURL, token, fetch: fetcher = fetch }) {
  const response = await fetcher(`${contentURL.replace(/\/$/, '')}/preview`, { method: 'POST', headers: { authorization: `Bearer ${token}` }, cache: 'no-store', redirect: 'error' })
  if (!response.ok) throw new Error('Preview authorization expired or denied')
  return response.json()
}
