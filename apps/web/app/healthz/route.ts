export const dynamic = 'force-dynamic'

export async function GET() {
  return Response.json(
    { status: 'ok', service: 'argus-web' },
    { headers: { 'Cache-Control': 'no-store' } },
  )
}
