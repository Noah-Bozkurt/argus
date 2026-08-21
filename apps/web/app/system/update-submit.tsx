'use client'

import { useState } from 'react'

export default function UpdateSubmit({ disabled }: { disabled: boolean }) {
  const [pending, setPending] = useState(false)

  return (
    <button
      className="primary"
      type="submit"
      disabled={disabled || pending}
      aria-busy={pending}
      onClick={(event) => {
        if (event.currentTarget.form?.checkValidity()) setPending(true)
      }}
    >
      {pending ? 'Authorizing and scheduling…' : 'Run preflight and update'}
    </button>
  )
}
