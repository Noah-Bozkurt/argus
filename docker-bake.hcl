variable "RELEASE_SHA" {
  default = "dev"
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
  tags = ["ghcr.io/noah-bozkurt/argus-web:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-web"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=gha,scope=argus-web"]
  cache-to = ["type=gha,mode=max,scope=argus-web"]
}

target "content" {
  context = "."
  dockerfile = "deploy/docker/content.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-content:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-content"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=gha,scope=argus-content"]
  cache-to = ["type=gha,mode=max,scope=argus-content"]
}

target "control-api" {
  context = "."
  dockerfile = "deploy/docker/control-api.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-control-api:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-control-api"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=gha,scope=argus-control-api"]
  cache-to = ["type=gha,mode=max,scope=argus-control-api"]
}

target "worker" {
  context = "."
  dockerfile = "deploy/docker/worker.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-worker:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-worker"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
  }
  cache-from = ["type=gha,scope=argus-worker"]
  cache-to = ["type=gha,mode=max,scope=argus-worker"]
}

target "host-tools" {
  context = "."
  dockerfile = "deploy/docker/host-tools.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-host-tools:${RELEASE_SHA}"]
  labels = {
    "org.opencontainers.image.title" = "argus-host-tools"
    "org.opencontainers.image.source" = SOURCE_URL
    "org.opencontainers.image.revision" = RELEASE_SHA
    "org.opencontainers.image.version" = RELEASE_SHA
    "org.argus.update-runner-protocol" = "1"
    "org.argus.branch-update-protocol" = "1"
  }
  cache-from = ["type=gha,scope=argus-host-tools"]
  cache-to = ["type=gha,mode=max,scope=argus-host-tools"]
}
