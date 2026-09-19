import type { AstroIntegration } from 'astro'
export type ArgusRecord = { id: string; model: string; values: Record<string, unknown>; layout: Array<{ id: string; component: string; values: Record<string, unknown> }>; published_at: string | null; updated_at: string }
export type ArgusRelease = { id: string; snapshot: { version: 1; projectId: string; models: Record<string, { records: ArgusRecord[] }>; media: Record<string, unknown> } }
export function loadRelease(options: { contentURL: string; projectId: string; releaseId?: string; fetch?: typeof fetch }): Promise<ArgusRelease>
export function loadPreview(options: { contentURL: string; token: string; fetch?: typeof fetch }): Promise<{ projectId: string; allowed_components: string[]; record: { id: string; values: Record<string, unknown>; layout: ArgusRecord['layout']; model: string } }>
export default function argus(options?: { enabled?: boolean; contentURL?: string; projectId?: string; releaseId?: string; components?: unknown[] }): AstroIntegration
