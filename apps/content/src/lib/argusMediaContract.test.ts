import assert from 'node:assert/strict'
import test from 'node:test'

import { MAX_MEDIA_BYTES, normalizeMediaMetadata, validMediaFile } from './argusMediaContract.ts'

test('normalizes bounded media metadata and explicit public visibility', () => {
  assert.deepEqual(normalizeMediaMetadata({ alt: ' Hero ', caption: ' Welcome ', public_read: 'true' }), {
    alt: 'Hero', caption: 'Welcome', publicRead: true,
  })
  assert.equal(normalizeMediaMetadata({ alt: '' }), null)
  assert.equal(normalizeMediaMetadata({ alt: 'x'.repeat(301) }), null)
})

test('accepts only bounded supported image uploads', () => {
  assert.equal(validMediaFile({ size: 1, type: 'image/png' }), true)
  assert.equal(validMediaFile({ size: MAX_MEDIA_BYTES, type: 'image/avif' }), true)
  assert.equal(validMediaFile({ size: MAX_MEDIA_BYTES + 1, type: 'image/jpeg' }), false)
  assert.equal(validMediaFile({ size: 100, type: 'image/svg+xml' }), false)
  assert.equal(validMediaFile({ size: 0, type: 'image/png' }), false)
})
