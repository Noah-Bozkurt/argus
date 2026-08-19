export const dynamic = 'force-dynamic'

export async function GET() {
  return Response.json(
    { status: 'ok', service: 'argus-content' },
    { headers: { 'Cache-Control': 'no-store' } },
  )
}
