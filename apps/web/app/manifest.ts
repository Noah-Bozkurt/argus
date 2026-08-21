import type { MetadataRoute } from 'next'

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: 'Argus Control Plane',
    short_name: 'Argus',
    description: 'Projects, infrastructure and operations in one self-hosted control plane.',
    start_url: '/',
    display: 'standalone',
    background_color: '#0b0e14',
    theme_color: '#0b0e14',
    orientation: 'any',
    icons: [
      { src: '/argus-icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' },
      { src: '/argus-icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'maskable' },
    ],
  }
}
