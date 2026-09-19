import { NextResponse } from 'next/server'
import { siteRequest } from '../../../../../lib/content-api'
export async function POST(request: Request, { params }: { params: { projectId: string } }) {
  // Browser mutations must originate from this application's origin.
  const origin = request.headers.get('origin')
  const expected = process.env.ARGUS_PUBLIC_URL || new URL(request.url).origin
  if (!origin || origin !== new URL(expected).origin) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
  try {
    const { action, ...body } = await request.json()
    const path = action === 'preview' ? '/preview' : action === 'publish' ? '/releases' : action === 'retry' ? '/retry' : action === 'verify' ? '/verify' : action === 'configure' ? '' : null
    if (path === null) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
    return NextResponse.json(await siteRequest(params.projectId, path, body))
  } catch (error) {
    const safe = ['PERMISSION_DENIED', 'PROJECT_NOT_READY', 'INVALID_URL', 'INVALID_TARGET', 'SITE_ENCRYPTION_KEY_NOT_CONFIGURED', 'CLOUDFLARE_REQUEST_FAILED', 'SITE_NOT_CONNECTED', 'INVALID_REQUEST', 'PUBLIC_CONTENT_REQUIRED', 'RELEASE_IN_PROGRESS']
    const message = error instanceof Error ? error.message : ''
    return NextResponse.json({ code: safe.includes(message) ? message : 'SITE_REQUEST_FAILED' }, { status: 400 })
  }
}
