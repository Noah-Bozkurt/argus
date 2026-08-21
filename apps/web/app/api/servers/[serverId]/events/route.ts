import { getCommandHistory, getServer } from '../../../../../lib/api'

export const dynamic = 'force-dynamic'

export async function GET(_: Request, { params }: { params: { serverId: string } }) {
  const [server, commands] = await Promise.all([
    getServer(params.serverId),
    getCommandHistory(params.serverId),
  ])
  const payload = JSON.stringify({ server, commands })
  return new Response(`retry: 5000\nevent: snapshot\ndata: ${payload}\n\n`, {
    headers: {
      'content-type': 'text/event-stream',
      'cache-control': 'no-cache, no-transform',
      connection: 'keep-alive',
    },
  })
}
