import { NextResponse } from 'next/server'
import { clearSessionCookie, currentSessionToken, revokeWorkspaceSession } from '../../../lib/auth'

export async function POST(request: Request) {
  const token = currentSessionToken()
  if (token) await revokeWorkspaceSession(token)
  clearSessionCookie()
  return NextResponse.redirect(new URL('/login', request.url), { status: 303 })
}
