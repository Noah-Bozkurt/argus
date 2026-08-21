FROM node:22-bookworm-slim AS build

WORKDIR /workspace
RUN npm install --global pnpm@9.15.4

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/package.json
COPY apps/content/package.json apps/content/package.json
COPY apps/installer/package.json apps/installer/package.json
COPY packages/protocol-ts/package.json packages/protocol-ts/package.json
COPY packages/ui/package.json packages/ui/package.json
RUN pnpm install --frozen-lockfile

COPY apps/web apps/web
COPY packages packages
RUN pnpm --filter @argus/web run build

FROM node:22-bookworm-slim AS runtime

ENV NODE_ENV=production \
    HOSTNAME=0.0.0.0 \
    PORT=3000

RUN groupadd --system --gid 10001 argus \
    && useradd --system --uid 10001 --gid argus --home-dir /app argus

WORKDIR /app
COPY --from=build --chown=argus:argus /workspace/apps/web/.next/standalone/ ./
COPY --from=build --chown=argus:argus /workspace/apps/web/.next/static ./apps/web/.next/static

USER argus
WORKDIR /app/apps/web
EXPOSE 3000
CMD ["node", "server.js"]
