variable "RELEASE_SHA" {
  default = "dev"
}

variable "REGISTRY" {
  default = "ghcr.io/noah-bozkurt"
}

variable "SOURCE_URL" {
  default = "https://github.com/Noah-Bozkurt/argus"
}

group "default" {
  targets = ["web", "content", "control-api", "worker", "host-tools"]
}

target "web" {
  context = "."
  dockerfile = "deploy/docker/web.Dockerfile"
  tags = ["${REGISTRY}/argus-web:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-web"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=registry,ref=${REGISTRY}/argus-web:buildcache"]
  cache-to = ["type=registry,ref=${REGISTRY}/argus-web:buildcache,mode=max"]
}

target "content" {
  context = "."
  dockerfile = "deploy/docker/content.Dockerfile"
  tags = ["${REGISTRY}/argus-content:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-content"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=registry,ref=${REGISTRY}/argus-content:buildcache"]
  cache-to = ["type=registry,ref=${REGISTRY}/argus-content:buildcache,mode=max"]
}

target "control-api" {
  context = "."
  dockerfile = "deploy/docker/control-api.Dockerfile"
  tags = ["${REGISTRY}/argus-control-api:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-control-api"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=registry,ref=${REGISTRY}/argus-control-api:buildcache"]
  cache-to = ["type=registry,ref=${REGISTRY}/argus-control-api:buildcache,mode=max"]
}

target "worker" {
  context = "."
  dockerfile = "deploy/docker/worker.Dockerfile"
  tags = ["${REGISTRY}/argus-worker:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-worker"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=registry,ref=${REGISTRY}/argus-worker:buildcache"]
  cache-to = ["type=registry,ref=${REGISTRY}/argus-worker:buildcache,mode=max"]
}

target "host-tools" {
  context = "."
  dockerfile = "deploy/docker/host-tools.Dockerfile"
  tags = ["${REGISTRY}/argus-host-tools:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-host-tools"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
    "org.argus.update-runner-protocol" = "1"
    "org.argus.branch-update-protocol" = "1"
  }
  cache-from = ["type=registry,ref=${REGISTRY}/argus-host-tools:buildcache"]
  cache-to = ["type=registry,ref=${REGISTRY}/argus-host-tools:buildcache,mode=max"]
}
