'use server'

import { redirect } from 'next/navigation'
import { clearSessionCookie, isOperatorRole, loginWorkspace, writeSessionCookie } from '../../lib/auth'

function safeNext(value: FormDataEntryValue | null): string {
  if (typeof value !== 'string' || !value.startsWith('/') || value.startsWith('//')) return '/'
  if (value === '/login' || value.startsWith('/login?')) return '/'
  return value
}

function resolveEmail(identifier: string): string {
  const normalized = identifier.trim().toLowerCase()
  if (normalized.includes('@')) return normalized
  const alias = (process.env.ARGUS_LEGACY_LOGIN_ALIAS ?? 'argus').trim().toLowerCase()
  if (normalized === alias) {
    return (process.env.ARGUS_OPERATOR_EMAIL ?? 'operator@argus.local').trim().toLowerCase()
  }
  return normalized
}

export async function loginAction(formData: FormData): Promise<never> {
  const identifierValue = formData.get('identifier')
  const passwordValue = formData.get('password')
  const identifier = typeof identifierValue === 'string' ? identifierValue : ''
  const password = typeof passwordValue === 'string' ? passwordValue : ''
  const next = safeNext(formData.get('next'))

  if (!identifier.trim() || !password) redirect('/login?error=invalid')

  const session = await loginWorkspace(resolveEmail(identifier), password)
  if (!session) redirect('/login?error=invalid')

  writeSessionCookie(session.token, session.exp)

  if (session.user.role === 'client') {
    const cms = process.env.ARGUS_CONTENT_PUBLIC_URL
    if (!cms) {
      clearSessionCookie()
      redirect('/login?error=cms_unavailable')
    }
    redirect(new URL('/admin', cms).toString())
  }

  if (!isOperatorRole(session.user.role) || !session.user.argusUserId) {
    clearSessionCookie()
    redirect('/login?error=operator_access')
  }

  redirect(next)
}
