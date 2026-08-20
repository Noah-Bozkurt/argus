const path = require('path')

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  output: 'standalone',
  experimental: {
    outputFileTracingRoot: path.join(__dirname, '../..'),
    serverActions: {
      bodySizeLimit: '11mb',
    },
  },
}

module.exports = nextConfig
