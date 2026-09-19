import { mkdir, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

/** Fetch once, then hold the immutable snapshot for the entire build. */
export async function loadRelease({ contentURL, projectId, releaseId = 'build', fetch: fetcher = fetch }) {
  if (!contentURL || !projectId) throw new Error('ARGUS_CONTENT_PUBLIC_URL and ARGUS_PROJECT_ID are required')
  const response = await fetcher(`${contentURL.replace(/\/$/, '')}/public/projects/${encodeURIComponent(projectId)}/releases/${encodeURIComponent(releaseId)}`, { cache: 'no-store' })
  if (!response.ok) throw new Error(`Argus release unavailable (${response.status}); refusing a partial build`)
  const release = await response.json()
  if (!release.id || release.snapshot?.version !== 1 || release.snapshot.projectId !== projectId || !release.snapshot.models) throw new Error('Invalid Argus release')
  return release
}

/** Generic Astro integration. Component rendering belongs to the consuming site. */
export default function argus(options = {}) {
  let release = null
  const enabled = options.enabled ?? process.env.ARGUS_CONTENT_MODE === 'cms'
  const projectId = options.projectId ?? process.env.ARGUS_PROJECT_ID
  const contentURL = options.contentURL ?? process.env.ARGUS_CONTENT_PUBLIC_URL
  return {
    name: '@argus/astro',
    hooks: {
      'astro:config:setup': async ({ updateConfig }) => {
        if (enabled) release = await loadRelease({ contentURL, projectId, releaseId: options.releaseId ?? process.env.ARGUS_RELEASE_ID ?? 'build' })
        updateConfig({ vite: { plugins: [{
          name: 'argus-release',
          resolveId(id) { if (id === 'virtual:argus-content') return '\0virtual:argus-content' },
          load(id) { if (id === '\0virtual:argus-content') return `export default ${JSON.stringify(release)};` },
        }] } })
      },
      'astro:build:done': async ({ dir }) => {
        const output = fileURLToPath(dir)
        await mkdir(output, { recursive: true })
        await writeFile(new URL('argus-components.json', dir), JSON.stringify({ version: 1, components: options.components ?? [] }))
        if (release) await writeFile(new URL('argus-release.json', dir), JSON.stringify({ projectId, releaseId: release.id }))
      },
    },
  }
}

export { loadPreview } from './runtime.mjs'
