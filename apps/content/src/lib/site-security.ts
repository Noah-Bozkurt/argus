import { createCipheriv, createDecipheriv, createHmac, randomBytes, timingSafeEqual } from 'node:crypto'

function key(): Buffer {
  const raw = process.env.ARGUS_SITE_ENCRYPTION_KEY ?? ''
  if (!/^[a-f0-9]{64}$/i.test(raw)) throw new Error('SITE_ENCRYPTION_KEY_NOT_CONFIGURED')
  return Buffer.from(raw, 'hex')
}
export function seal(value: string, scope: string): string {
  const iv = randomBytes(12)
  const cipher = createCipheriv('aes-256-gcm', key(), iv)
  cipher.setAAD(Buffer.from(scope))
  const encrypted = Buffer.concat([cipher.update(value, 'utf8'), cipher.final()])
  return [iv, cipher.getAuthTag(), encrypted].map(part => part.toString('base64url')).join('.')
}
export function unseal(value: string, scope: string): string {
  const parts = value.split('.').map(part => Buffer.from(part, 'base64url'))
  if (parts.length !== 3) throw new Error('INVALID_CREDENTIAL')
  const cipher = createDecipheriv('aes-256-gcm', key(), parts[0])
  cipher.setAAD(Buffer.from(scope))
  cipher.setAuthTag(parts[1])
  return Buffer.concat([cipher.update(parts[2]), cipher.final()]).toString('utf8')
}
export function httpsURL(raw: unknown): string {
  if (typeof raw !== 'string' || raw.length > 2048) throw new Error('INVALID_URL')
  const url = new URL(raw)
  if (url.protocol !== 'https:' || url.username || url.password || url.hash || url.search || url.port || url.hostname === 'localhost' || !url.hostname.includes('.') || /^[\d.]+$/.test(url.hostname) || url.hostname.includes(':')) throw new Error('INVALID_URL')
  return url.href.replace(/\/$/, '')
}
export type PreviewGrant = { project: string; record: string; user: string; origin: string; exp: number }
export function previewToken(grant: Omit<PreviewGrant, 'exp'>): string {
  const body = Buffer.from(JSON.stringify({ ...grant, exp: Math.floor(Date.now() / 1000) + 300 })).toString('base64url')
  return `${body}.${createHmac('sha256', key()).update(`preview:${body}`).digest('base64url')}`
}
export function verifyPreview(token: string): PreviewGrant | null {
  try {
    const [body, signature, extra] = token.split('.')
    if (extra || !body || !signature) return null
    const expected = createHmac('sha256', key()).update(`preview:${body}`).digest()
    const supplied = Buffer.from(signature, 'base64url')
    if (supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) return null
    const grant = JSON.parse(Buffer.from(body, 'base64url').toString('utf8')) as PreviewGrant
    return grant.exp > Date.now() / 1000 ? grant : null
  } catch { return null }
}
