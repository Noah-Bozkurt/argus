variable "RELEASE_SHA" {
  default = "dev"
}

group "default" {
  targets = ["web", "content", "control-api", "worker", "host-tools"]
}

target "web" {
  context = "."
  dockerfile = "deploy/docker/web.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-web:${RELEASE_SHA}"]
  cache-from = ["type=gha,scope=argus-web"]
  cache-to = ["type=gha,mode=max,scope=argus-web"]
}

target "content" {
  context = "."
  dockerfile = "deploy/docker/content.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-content:${RELEASE_SHA}"]
  cache-from = ["type=gha,scope=argus-content"]
  cache-to = ["type=gha,mode=max,scope=argus-content"]
}

target "control-api" {
  context = "."
  dockerfile = "deploy/docker/control-api.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-control-api:${RELEASE_SHA}"]
  cache-from = ["type=gha,scope=argus-control-api"]
  cache-to = ["type=gha,mode=max,scope=argus-control-api"]
}

target "worker" {
  context = "."
  dockerfile = "deploy/docker/worker.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-worker:${RELEASE_SHA}"]
  cache-from = ["type=gha,scope=argus-worker"]
  cache-to = ["type=gha,mode=max,scope=argus-worker"]
}

target "host-tools" {
  context = "."
  dockerfile = "deploy/docker/host-tools.Dockerfile"
  tags = ["ghcr.io/noah-bozkurt/argus-host-tools:${RELEASE_SHA}"]
  cache-from = ["type=gha,scope=argus-host-tools"]
  cache-to = ["type=gha,mode=max,scope=argus-host-tools"]
}
