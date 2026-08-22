// Local source builds deliberately avoid GitHub Actions cache exporters.
// The transactional updater overlays this file on docker-bake.hcl when a
// branch is built directly on an Argus host.

target "web" {
  cache-from = []
  cache-to = []
}

target "content" {
  cache-from = []
  cache-to = []
}

target "control-api" {
  cache-from = []
  cache-to = []
}

target "worker" {
  cache-from = []
  cache-to = []
}

target "host-tools" {
  cache-from = []
  cache-to = []
}
