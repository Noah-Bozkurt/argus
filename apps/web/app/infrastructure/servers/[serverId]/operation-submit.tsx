'use client'

import { useFormStatus } from 'react-dom'

export default function OperationSubmit({ children, disabled = false, className }: { children: string; disabled?: boolean; className?: string }) {
  const { pending } = useFormStatus()
  return <button className={className} type="submit" disabled={disabled || pending} aria-busy={pending}>{pending ? 'Queuing…' : children}</button>
}
