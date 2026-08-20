import { NextResponse } from 'next/server'
import { createEnrollmentToken, createServer } from '../../../../../lib/api'

export async function POST(request: Request, { params }: { params: { projectId: string } }) {
  try {
    const input = await request.json() as { environment_id?: string; hostname?: string }
    if (!input.environment_id || !input.hostname?.trim()) return NextResponse.json({ message: 'Environment and hostname are required.' }, { status: 400 })
    const publicUrl = process.env.ARGUS_PUBLIC_URL
    if (!publicUrl?.startsWith('https://') && process.env.NODE_ENV !== 'development') return NextResponse.json({ message: 'ARGUS_PUBLIC_URL must use HTTPS.' }, { status: 500 })
    const server = await createServer(params.projectId, input.environment_id, input.hostname.trim())
    const enrollment = await createEnrollmentToken(server.server_id)
    const setupCode = Buffer.from(JSON.stringify({ version: 1, control_plane_url: publicUrl ?? 'http://localhost:8080', server_id: server.server_id, enrollment_token: enrollment.token, expires_at: enrollment.expires_at })).toString('base64')
    return NextResponse.json({ setup_code: setupCode }, { headers: { 'cache-control': 'no-store' } })
  } catch (error) {
    return NextResponse.json({ message: error instanceof Error ? error.message : 'Could not create server.' }, { status: 500 })
  }
}
