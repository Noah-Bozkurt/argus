FROM node:22-bookworm-slim AS build

WORKDIR /workspace
RUN npm install --global pnpm@9.15.4
COPY . .
RUN pnpm install --frozen-lockfile
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
