import { getServers } from '../../../lib/api'

export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    return Response.json(await getServers(), {
      headers: { 'cache-control': 'no-store' },
    })
  } catch (error) {
    console.error('Unable to load server fleet', error)
    return Response.json({ code: 'CONTROL_API_UNAVAILABLE', message: 'Unable to load servers' }, { status: 503 })
  }
}
