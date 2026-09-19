import { lookup } from 'node:dns/promises'
import { request } from 'node:https'
import { isIP } from 'node:net'
import { httpsURL } from './site-security'

function publicAddress(address: string): boolean {
  if (isIP(address) !== 4) return false
  const [a, b] = address.split('.').map(Number)
  return a > 0 && a !== 10 && a !== 127 && a < 224 && !(a === 169 && b === 254) && !(a === 172 && b >= 16 && b <= 31) && !(a === 192 && (b === 168 || b === 0)) && !(a === 100 && b >= 64 && b <= 127) && !(a === 198 && (b === 18 || b === 19))
}
// Pin a validated DNS answer to prevent SSRF/rebinding during connection checks.
export async function publicSiteJSON(raw: string): Promise<unknown> {
  const url = new URL(httpsURL(raw))
  const answers = await lookup(url.hostname, { all: true, family: 4 })
  if (!answers.length || answers.some(answer => !publicAddress(answer.address))) throw new Error('INVALID_URL')
  return new Promise((resolve, reject) => {
    const req = request(url, { timeout: 8000, headers: { accept: 'application/json' }, lookup: (_hostname, options, callback) => {
      if (typeof options === 'object' && options.all) callback(null, [answers[0]])
      else callback(null, answers[0].address, 4)
    } }, response => {
      if (response.statusCode !== 200) { response.resume(); reject(new Error('SITE_CHECK_FAILED')); return }
      const chunks: Buffer[] = []; let size = 0
      response.on('data', (chunk: Buffer) => {
        size += chunk.length
        if (size > 1024 * 1024) req.destroy(new Error('SITE_RESPONSE_TOO_LARGE'))
        else chunks.push(chunk)
      })
      response.on('end', () => { try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8'))) } catch { reject(new Error('SITE_CHECK_FAILED')) } })
      response.on('error', reject)
    })
    req.on('timeout', () => req.destroy(new Error('SITE_CHECK_TIMEOUT')))
    req.on('error', reject)
    req.end()
  })
}
