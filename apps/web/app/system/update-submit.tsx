'use client'

import { useFormStatus } from 'react-dom'

export default function UpdateSubmit({ disabled }: { disabled: boolean }) {
  const { pending } = useFormStatus()
  return <button className="primary" type="submit" disabled={disabled || pending} aria-busy={pending}>{pending ? 'Authorizing and scheduling…' : 'Run preflight and update'}</button>
}
