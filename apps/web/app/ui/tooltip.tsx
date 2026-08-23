'use client'

import * as TooltipPrimitive from '@radix-ui/react-tooltip'
import type { ReactElement } from 'react'

export default function Tooltip({ content, children }: { content: string; children: ReactElement }) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content className="argus-tooltip" sideOffset={6}>
          {content}
          <TooltipPrimitive.Arrow className="argus-tooltip-arrow" />
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  )
}
