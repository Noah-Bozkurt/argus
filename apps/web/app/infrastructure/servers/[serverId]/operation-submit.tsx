'use client'

import { useState } from 'react'

export default function OperationSubmit({ children, disabled = false, className }: { children: string; disabled?: boolean; className?: string }) {
  const [pending, setPending] = useState(false)

  return (
    <button
      className={className}
      type="submit"
      disabled={disabled || pending}
      aria-busy={pending}
      onClick={(event) => {
        if (event.currentTarget.form?.checkValidity()) setPending(true)
      }}
    >
      {pending ? 'Queuing…' : children}
    </button>
  )
}
