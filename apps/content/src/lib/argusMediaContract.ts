export const MAX_MEDIA_BYTES = 10 * 1024 * 1024
export const MEDIA_MIME_TYPES = new Set(['image/jpeg', 'image/png', 'image/webp', 'image/avif'])

export function normalizeMediaMetadata(input: { alt?: unknown; caption?: unknown; public_read?: unknown }) {
  const alt = typeof input.alt === 'string' ? input.alt.trim() : ''
  const caption = typeof input.caption === 'string' ? input.caption.trim() : ''
  if (!alt || alt.length > 300 || caption.length > 2000) return null
  return { alt, caption, publicRead: input.public_read === true || input.public_read === 'true' }
}

export function validMediaFile(file: { size: number; type: string }): boolean {
  return file.size > 0 && file.size <= MAX_MEDIA_BYTES && MEDIA_MIME_TYPES.has(file.type)
}
