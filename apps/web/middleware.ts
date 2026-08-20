import { NextRequest, NextResponse } from 'next/server'

const SESSION_COOKIE = 'payload-token'
const contentApi = process.env.ARGUS_CONTENT_URL ?? 'http://content:3000'
const contentPublicUrl = process.env.ARGUS_CONTENT_PUBLIC_URL ?? ''

type WorkspaceRole = 'owner' | 'admin' | 'member' | 'client'
type SessionUser = { role?: WorkspaceRole; argusUserId?: string | null }

function publicPath(pathname: string): boolean {
  return pathname === '/healthz' || pathname.startsWith('/status/') || pathname === '/login'
}

async function userForToken(token: string): Promise<SessionUser | null> {
  try {
    const response = await fetch(`${contentApi}/api/workspace-users/me`, {
      cache: 'no-store',
      headers: { authorization: `JWT ${token}` },
    })
    if (!response.ok) return null
    const result = (await response.json()) as { user?: SessionUser | null }
    return result.user ?? null
  } catch {
    return null
  }
}

function loginRedirect(request: NextRequest, error?: string) {
  const login = new URL('/login', request.url)
  const next = `${request.nextUrl.pathname}${request.nextUrl.search}`
  if (next !== '/') login.searchParams.set('next', next)
  if (error) login.searchParams.set('error', error)
  return NextResponse.redirect(login)
}

export async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl
  const token = request.cookies.get(SESSION_COOKIE)?.value

  if (!token) {
    return publicPath(pathname) ? NextResponse.next() : loginRedirect(request)
  }

  const user = await userForToken(token)
  if (!user?.role) {
    if (publicPath(pathname)) {
      const response = NextResponse.next()
      response.cookies.delete(SESSION_COOKIE)
      return response
    }
    const response = loginRedirect(request)
    response.cookies.delete(SESSION_COOKIE)
    return response
  }

  if (user.role === 'client') {
    if (pathname === '/login' || !publicPath(pathname)) {
      if (contentPublicUrl) return NextResponse.redirect(new URL('/admin', contentPublicUrl))
      return new NextResponse('Client CMS access is not configured', { status: 503 })
    }
    return NextResponse.next()
  }

  if (!user.argusUserId) {
    const response = loginRedirect(request, 'operator_access')
    response.cookies.delete(SESSION_COOKIE)
    return response
  }

  if (pathname === '/login') return NextResponse.redirect(new URL('/', request.url))
  return NextResponse.next()
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
}
