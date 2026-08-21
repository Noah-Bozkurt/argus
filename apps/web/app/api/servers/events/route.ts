import { getServers } from '../../../../lib/api'

export const dynamic = 'force-dynamic'

export async function GET() {
  const servers = await getServers()
  return new Response(`retry: 5000\nevent: snapshot\ndata: ${JSON.stringify(servers)}\n\n`, {
    headers: { 'content-type': 'text/event-stream', 'cache-control': 'no-cache, no-transform', connection: 'keep-alive' },
  })
}
