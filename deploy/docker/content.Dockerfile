FROM node:22-bookworm-slim AS build

WORKDIR /workspace
RUN npm install --global pnpm@9.15.4
COPY . .
RUN pnpm install --frozen-lockfile

# Payload loads its configuration during `next build`. These values exist only in
# the build stage so no production credential is baked into the runtime image.
# The build must not depend on a reachable database; committed migrations run at
# production startup against the real DATABASE_URL supplied by Compose.
ENV NODE_ENV=production \
    DATABASE_URL=postgresql://argus:build-only@127.0.0.1:9/argus \
    ARGUS_CONTENT_DB_SCHEMA=argus_content \
    PAYLOAD_SECRET=00000000000000000000000000000000 \
    PAYLOAD_PUBLIC_URL=http://localhost:3000 \
    ARGUS_CONTENT_SYNC_TOKEN=00000000000000000000000000000000 \
    PAYLOAD_DB_PUSH=false
RUN pnpm --filter @argus/content run build

FROM node:22-bookworm-slim AS runtime

ENV NODE_ENV=production \
    HOSTNAME=0.0.0.0 \
    PORT=3000

RUN groupadd --system --gid 10001 argus \
    && useradd --system --uid 10001 --gid argus --home-dir /app argus

WORKDIR /app
COPY --from=build --chown=argus:argus /workspace/apps/content/.next/standalone/ ./
COPY --from=build --chown=argus:argus /workspace/apps/content/.next/static ./apps/content/.next/static

USER argus
WORKDIR /app/apps/content
EXPOSE 3000
CMD ["node", "server.js"]
