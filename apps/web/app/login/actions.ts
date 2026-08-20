'use server'

import { redirect } from 'next/navigation'
import { clearSessionCookie, isOperatorRole, loginWorkspace, writeSessionCookie } from '../../lib/auth'

function safeNext(value: FormDataEntryValue | null): string {
  if (typeof value !== 'string' || !value.startsWith('/') || value.startsWith('//')) return '/'
  if (value === '/login' || value.startsWith('/login?')) return '/'
  return value
}

export async function loginAction(formData: FormData): Promise<never> {
  const emailValue = formData.get('email')
  const passwordValue = formData.get('password')
  const email = typeof emailValue === 'string' ? emailValue.trim().toLowerCase() : ''
  const password = typeof passwordValue === 'string' ? passwordValue : ''
  const next = safeNext(formData.get('next'))

  if (!email || !password) redirect('/login?error=invalid')

  const session = await loginWorkspace(email, password)
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
