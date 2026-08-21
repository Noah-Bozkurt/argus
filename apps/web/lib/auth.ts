import { cookies } from 'next/headers'

export const SESSION_COOKIE = 'payload-token'

export type WorkspaceRole = 'owner' | 'admin' | 'member' | 'client'

export type SessionUser = {
  id: string
  email: string
  displayName?: string | null
  organizationId?: string | null
  argusUserId?: string | null
  role: WorkspaceRole
}

type LoginResponse = {
  token?: string
  exp?: number
  user?: unknown
}

type MeResponse = {
  user?: unknown
  exp?: number
}

const contentApi = process.env.ARGUS_CONTENT_URL ?? 'http://content:3000'

function sessionUser(value: unknown): SessionUser | null {
  if (!value || typeof value !== 'object') return null
  const user = value as Record<string, unknown>
  const role = user.role
  if (
    typeof user.id !== 'string' ||
    typeof user.email !== 'string' ||
    !['owner', 'admin', 'member', 'client'].includes(String(role))
  ) {
    return null
  }
  return {
    id: user.id,
    email: user.email,
    displayName: typeof user.displayName === 'string' ? user.displayName : null,
    organizationId: typeof user.organizationId === 'string' ? user.organizationId : null,
    argusUserId: typeof user.argusUserId === 'string' ? user.argusUserId : null,
    role: role as WorkspaceRole,
  }
}

function cookieSettings(maxAge?: number) {
  const configuredDomain = process.env.ARGUS_AUTH_COOKIE_DOMAIN?.trim()
  return {
    httpOnly: true,
    secure: (process.env.ARGUS_PUBLIC_URL ?? '').startsWith('https://'),
    sameSite: 'lax' as const,
    path: '/',
    ...(configuredDomain ? { domain: configuredDomain } : {}),
    ...(typeof maxAge === 'number' ? { maxAge } : {}),
  }
}

export function isOperatorRole(role: WorkspaceRole): boolean {
  return role === 'owner' || role === 'admin' || role === 'member'
}

export async function loginWorkspace(
  email: string,
  password: string,
): Promise<{ token: string; exp?: number; user: SessionUser } | null> {
  const response = await fetch(`${contentApi}/api/workspace-users/login`, {
    method: 'POST',
    cache: 'no-store',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  if (!response.ok) return null

  const result = (await response.json()) as LoginResponse
  const user = sessionUser(result.user)
  if (!result.token || !user) return null
  return { token: result.token, exp: result.exp, user }
}

export async function getWorkspaceUser(token: string): Promise<SessionUser | null> {
  try {
    const response = await fetch(`${contentApi}/api/workspace-users/me`, {
      cache: 'no-store',
      headers: { authorization: `JWT ${token}` },
    })
    if (!response.ok) return null
    const result = (await response.json()) as MeResponse
    return sessionUser(result.user)
  } catch {
    return null
  }
}

export async function revokeWorkspaceSession(token: string): Promise<void> {
  try {
    await fetch(`${contentApi}/api/workspace-users/logout`, {
      method: 'POST',
      cache: 'no-store',
      headers: { authorization: `JWT ${token}` },
    })
  } catch {
    // Clearing the local cookie still signs this browser out if the auth service is unavailable.
  }
}

export function currentSessionToken(): string | null {
  return cookies().get(SESSION_COOKIE)?.value ?? null
}

export function writeSessionCookie(token: string, exp?: number): void {
  const nowSeconds = Math.floor(Date.now() / 1000)
  const maxAge = exp && exp > nowSeconds ? exp - nowSeconds : 8 * 60 * 60
  cookies().set(SESSION_COOKIE, token, cookieSettings(maxAge))
}

export function clearSessionCookie(): void {
  cookies().set(SESSION_COOKIE, '', cookieSettings(0))
}
