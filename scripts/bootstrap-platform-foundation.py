from pathlib import Path
import json
import re

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if old not in text:
        raise RuntimeError(f"expected source pattern missing from {path}: {old[:100]!r}")
    write(path, text.replace(old, new, 1))


# Rust workspace and Control API dependencies.
replace_once(
    "Cargo.toml",
    'tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }\nuuid = { version = "1.10", features = ["serde", "v4"] }',
    'tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }\ntower-http = { version = "0.6.11", features = ["request-id", "trace"] }\nutoipa = { version = "5.5.0", features = ["chrono", "uuid"] }\nuuid = { version = "1.10", features = ["serde", "v4"] }',
)
replace_once(
    "crates/protocol/Cargo.toml",
    "license.workspace = true\n\n[dependencies]",
    'license.workspace = true\n\n[features]\ndefault = []\nopenapi = ["dep:utoipa"]\n\n[dependencies]',
)
replace_once(
    "crates/protocol/Cargo.toml",
    'thiserror = { workspace = true }\nuuid = { workspace = true }',
    'thiserror = { workspace = true }\nutoipa = { workspace = true, optional = true }\nuuid = { workspace = true }',
)
replace_once(
    "services/control-api/Cargo.toml",
    'protocol = { path = "../../crates/protocol" }',
    'protocol = { path = "../../crates/protocol", features = ["openapi"] }',
)
replace_once(
    "services/control-api/Cargo.toml",
    'tracing-subscriber = { workspace = true }\nuuid = { workspace = true }',
    'tracing-subscriber = { workspace = true }\ntower-http = { workspace = true }\nutoipa = { workspace = true }\nuuid = { workspace = true }',
)

# Only expose protocol DTOs to utoipa when the Control API opts into the feature.
protocol_path = "crates/protocol/src/lib.rs"
protocol = read(protocol_path)
schema_types = [
    "Capability",
    "UpdateState",
    "PackageUpdate",
    "ServiceJournal",
    "DiagnosticsState",
    "DockerContainer",
    "DockerState",
    "SecurityFinding",
    "SecurityState",
    "BackupArtifact",
    "BackupState",
    "MountState",
    "NetworkInterfaceState",
    "ProcessState",
    "SystemSnapshot",
    "ServiceState",
]
for name in schema_types:
    pattern = rf'(#\[derive\([^\n]+\)\]\n)(pub (?:struct|enum) {name}\b)'
    replacement = rf'\1#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]\n\2'
    protocol, count = re.subn(pattern, replacement, protocol, count=1)
    if count != 1:
        raise RuntimeError(f"could not add OpenAPI schema derive to {name}")
write(protocol_path, protocol)

replace_once(
    "services/control-api/src/persistence.rs",
    "#[derive(Debug, Clone, Serialize)]\npub struct ServerView {",
    "#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]\npub struct ServerView {",
)

# Add a generated OpenAPI contract and tower-http tracing/request IDs without changing existing routes.
main_path = "services/control-api/src/main.rs"
main = read(main_path)
main = main.replace(
    "use tracing::info;\nuse uuid::Uuid;",
    "use tower_http::{\n    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},\n    trace::TraceLayer,\n};\nuse tracing::info;\nuse utoipa::OpenApi;\nuse uuid::Uuid;",
    1,
)
main = main.replace(
    "#[derive(Debug, Serialize)]\nstruct ErrorResponse {",
    "#[derive(Debug, Serialize, utoipa::ToSchema)]\nstruct ErrorResponse {",
    1,
)
main = main.replace(
    "#[derive(Debug, Deserialize)]\nstruct CreateServerRequest {",
    "#[derive(Debug, Deserialize, utoipa::ToSchema)]\nstruct CreateServerRequest {",
    1,
)
main = main.replace(
    "#[derive(Debug, Serialize)]\nstruct CreateServerResponse {",
    "#[derive(Debug, Serialize, utoipa::ToSchema)]\nstruct CreateServerResponse {",
    1,
)
maintenance_marker = """#[derive(Debug, Deserialize)]
struct StartMaintenanceRequest {
    duration_minutes: i64,
    reason: String,
}
"""
if maintenance_marker not in main:
    raise RuntimeError("missing StartMaintenanceRequest marker")
main = main.replace(
    maintenance_marker,
    maintenance_marker
    + """
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(health, list_servers, create_server, get_server),
    components(schemas(
        ErrorResponse,
        CreateServerRequest,
        CreateServerResponse,
        persistence::ServerView
    )),
    tags(
        (name = "system", description = "Control-plane health and metadata"),
        (name = "servers", description = "Managed server inventory")
    )
)]
struct ApiDoc;
""",
    1,
)
main = main.replace(
    "async fn main() -> anyhow::Result<()> {\n    tracing_subscriber::fmt()",
    "async fn main() -> anyhow::Result<()> {\n    if std::env::args().any(|arg| arg == \"--print-openapi\") {\n        println!(\"{}\", serde_json::to_string_pretty(&ApiDoc::openapi())?);\n        return Ok(());\n    }\n\n    tracing_subscriber::fmt()",
    1,
)
main = main.replace(
    '.route("/health", get(health))',
    '.route("/health", get(health))\n        .route("/openapi.json", get(openapi_json))',
    1,
)
main = main.replace(
    "        .merge(jobs_admin::router())\n        .with_state(state);",
    "        .merge(jobs_admin::router())\n        .with_state(state)\n        .layer(PropagateRequestIdLayer::x_request_id())\n        .layer(TraceLayer::new_for_http())\n        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));",
    1,
)
main = main.replace(
    "async fn health() -> &'static str {\n    \"ok\"\n}",
    """#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses((status = 200, description = "Control API is healthy"))
)]
async fn health() -> &'static str {
    "ok"
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}""",
    1,
)
main = main.replace(
    "async fn list_servers(\n",
    """#[utoipa::path(
    get,
    path = "/servers",
    tag = "servers",
    responses(
        (status = 200, description = "Managed servers", body = [persistence::ServerView]),
        (status = 401, description = "Missing or invalid authentication", body = ErrorResponse),
        (status = 403, description = "Identity is not allowed", body = ErrorResponse)
    )
)]
async fn list_servers(
""",
    1,
)
main = main.replace(
    "async fn get_server(\n",
    """#[utoipa::path(
    get,
    path = "/servers/{server_id}",
    tag = "servers",
    params(("server_id" = Uuid, Path, description = "Managed server identifier")),
    responses(
        (status = 200, description = "Managed server", body = persistence::ServerView),
        (status = 401, description = "Missing or invalid authentication", body = ErrorResponse),
        (status = 403, description = "Identity is not allowed", body = ErrorResponse),
        (status = 404, description = "Server not found", body = ErrorResponse)
    )
)]
async fn get_server(
""",
    1,
)
main = main.replace(
    "async fn create_server(\n",
    """#[utoipa::path(
    post,
    path = "/servers",
    tag = "servers",
    request_body = CreateServerRequest,
    responses(
        (status = 200, description = "Server created", body = CreateServerResponse),
        (status = 400, description = "Invalid server request", body = ErrorResponse),
        (status = 401, description = "Missing or invalid authentication", body = ErrorResponse),
        (status = 403, description = "Identity is not allowed", body = ErrorResponse)
    )
)]
async fn create_server(
""",
    1,
)
main += """

#[cfg(test)]
mod openapi_tests {
    use super::*;

    #[test]
    fn core_server_contract_is_exported() {
        let value = serde_json::to_value(ApiDoc::openapi()).expect("serialize OpenAPI");
        let paths = value["paths"].as_object().expect("OpenAPI paths");
        assert!(paths.contains_key("/health"));
        assert!(paths.contains_key("/servers"));
        assert!(paths.contains_key("/servers/{server_id}"));
    }
}
"""
write(main_path, main)

# Package scripts and frontend dependencies.
root_pkg = json.loads(read("package.json"))
root_pkg["scripts"]["generate:api"] = "bash scripts/generate-api-contracts.sh"
write("package.json", json.dumps(root_pkg, indent=2) + "\n")

web_pkg = json.loads(read("apps/web/package.json"))
web_pkg["scripts"]["generate:api"] = "openapi-typescript ./openapi/control-api.json -o ./lib/generated/control-api.ts"
web_pkg["dependencies"].update(
    {
        "@radix-ui/react-tooltip": "1.2.16",
        "@tanstack/react-query": "5.101.4",
        "@tanstack/react-table": "8.21.3",
    }
)
web_pkg["devDependencies"]["openapi-typescript"] = "7.13.0"
write("apps/web/package.json", json.dumps(web_pkg, indent=2) + "\n")

write(
    "scripts/generate-api-contracts.sh",
    """#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
spec="$root/apps/web/openapi/control-api.json"
generated="$root/apps/web/lib/generated/control-api.ts"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

mkdir -p "$(dirname "$spec")" "$(dirname "$generated")"
(
  cd "$root"
  cargo run --quiet -p control-api -- --print-openapi > "$tmp"
)
mv "$tmp" "$spec"
pnpm --dir "$root/apps/web" run generate:api
""",
)

write(
    "apps/web/app/providers.tsx",
    """'use client'

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import * as Tooltip from '@radix-ui/react-tooltip'
import { useState, type ReactNode } from 'react'

export default function AppProviders({ children }: { children: ReactNode }) {
  const [queryClient] = useState(() => new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 5_000,
        retry: 1,
        refetchOnWindowFocus: true,
      },
    },
  }))

  return (
    <QueryClientProvider client={queryClient}>
      <Tooltip.Provider delayDuration={300} skipDelayDuration={100}>
        {children}
      </Tooltip.Provider>
    </QueryClientProvider>
  )
}
""",
)

write(
    "apps/web/app/ui/tooltip.tsx",
    """'use client'

import * as TooltipPrimitive from '@radix-ui/react-tooltip'
import type { ReactElement } from 'react'

export default function Tooltip({ content, children }: { content: string; children: ReactElement }) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content className="argus-tooltip" sideOffset={6}>
          {content}
          <TooltipPrimitive.Arrow className="argus-tooltip-arrow" />
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  )
}
""",
)

replace_once(
    "apps/web/app/layout.tsx",
    "import AppShell from './app-shell'\n",
    "import AppShell from './app-shell'\nimport AppProviders from './providers'\n",
)
replace_once(
    "apps/web/app/layout.tsx",
    """      <body>
        <FormDirtyGuard />
        <WorkspaceMemory />
        <AppShell user={user}>{children}</AppShell>
      </body>""",
    """      <body>
        <AppProviders>
          <FormDirtyGuard />
          <WorkspaceMemory />
          <AppShell user={user}>{children}</AppShell>
        </AppProviders>
      </body>""",
)

write(
    "apps/web/app/api/servers/route.ts",
    """import { getServers } from '../../../lib/api'

export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    return Response.json(await getServers(), {
      headers: { 'cache-control': 'no-store' },
    })
  } catch (error) {
    console.error('Unable to load server fleet', error)
    return Response.json({ code: 'CONTROL_API_UNAVAILABLE', message: 'Unable to load servers' }, { status: 503 })
  }
}
""",
)

write(
    "apps/web/lib/control-api-contract.ts",
    """import type { components, paths } from './generated/control-api'

export type ControlApiPaths = paths
export type ControlApiSchemas = components['schemas']
export type ControlApiServerView = components['schemas']['ServerView']
""",
)

write(
    "apps/web/app/infrastructure/servers/server-fleet.tsx",
    """'use client'

import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { flexRender, getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import type { ControlApiServerView as ServerView } from '../../../lib/control-api-contract'
import LucideIcon from '../../lucide-icons'
import Tooltip from '../../ui/tooltip'
import usePersistentChoice from '../../use-persistent-choice'

const FAVORITES_KEY = 'argus:favorites:v1'
const SERVER_QUERY_KEY = ['servers'] as const
const STATUS_FILTERS = ['all', 'online', 'offline', 'attention'] as const
const SORTS = ['name', 'heartbeat', 'cpu', 'disk'] as const

type StatusFilter = typeof STATUS_FILTERS[number]
type SortChoice = typeof SORTS[number]

function Utilization({ value }: { value: number | undefined }) {
  const safe = typeof value === 'number' ? Math.max(0, Math.min(100, value)) : 0
  return <div className="utilization-cell"><div className="utilization-value"><span>{typeof value === 'number' ? `${Math.round(value)}%` : '—'}</span></div><div className="utilization-track"><div className="utilization-fill" style={{ width: `${safe}%` }} /></div></div>
}

function relativeTime(value: string | null | undefined): string {
  if (!value) return 'Never'
  const delta = Date.now() - new Date(value).getTime()
  const seconds = Math.max(0, Math.floor(delta / 1000))
  if (seconds < 60) return `${seconds}s ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
}

function attention(server: ServerView): boolean {
  const snapshot = server.snapshot
  return !server.online || Boolean(snapshot && (
    snapshot.disk_percent >= 85 ||
    snapshot.updates.reboot_required ||
    snapshot.diagnostics.failed_units.length > 0 ||
    snapshot.security.findings.some((finding) => ['HIGH', 'CRITICAL'].includes(finding.severity.toUpperCase()))
  ))
}

function initialFavorites(): string[] {
  if (typeof window === 'undefined') return []
  try { return JSON.parse(window.localStorage.getItem(FAVORITES_KEY) ?? '[]') as string[] } catch { return [] }
}

async function fetchServers(): Promise<ServerView[]> {
  const response = await fetch('/api/servers', { cache: 'no-store' })
  if (!response.ok) throw new Error(`Unable to refresh servers (${response.status})`)
  return response.json() as Promise<ServerView[]>
}

export default function ServerFleet({ initialServers }: { initialServers: ServerView[] }) {
  const [live, setLive] = useState(false)
  const [query, setQuery] = useState('')
  const [status, setStatus] = usePersistentChoice<StatusFilter>('argus:servers:status', 'all', STATUS_FILTERS)
  const [sort, setSort] = usePersistentChoice<SortChoice>('argus:servers:sort', 'name', SORTS)
  const [favorites, setFavorites] = useState<string[]>(initialFavorites)
  const [copied, setCopied] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const serverQuery = useQuery({
    queryKey: SERVER_QUERY_KEY,
    queryFn: fetchServers,
    initialData: initialServers,
    staleTime: 5_000,
    refetchInterval: live ? false : 10_000,
  })
  const servers = serverQuery.data

  useEffect(() => {
    let lastMessage = 0
    const source = new EventSource('/api/servers/events')
    source.addEventListener('snapshot', (event) => {
      const next = JSON.parse((event as MessageEvent).data) as ServerView[]
      queryClient.setQueryData(SERVER_QUERY_KEY, next)
      lastMessage = Date.now()
      setLive(true)
    })
    source.onerror = () => { if (Date.now() - lastMessage > 20_000) setLive(false) }
    const staleTimer = window.setInterval(() => { if (Date.now() - lastMessage > 20_000) setLive(false) }, 5_000)
    return () => { source.close(); window.clearInterval(staleTimer) }
  }, [queryClient])

  const online = servers.filter((server) => server.online).length
  const needsAttention = servers.filter(attention).length
  const containers = servers.reduce((sum, server) => sum + (server.snapshot?.docker.containers.length ?? 0), 0)

  const visible = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return servers
      .filter((server) => {
        if (status === 'online' && !server.online) return false
        if (status === 'offline' && server.online) return false
        if (status === 'attention' && !attention(server)) return false
        return true
      })
      .filter((server) => !normalized || [server.hostname, server.server_id, server.snapshot?.os ?? '', server.snapshot?.architecture ?? '', ...server.services.map((service) => service.name)].join(' ').toLowerCase().includes(normalized))
      .sort((left, right) => {
        const leftPinned = favorites.includes(`server:${left.server_id}`) ? 1 : 0
        const rightPinned = favorites.includes(`server:${right.server_id}`) ? 1 : 0
        if (leftPinned !== rightPinned) return rightPinned - leftPinned
        if (sort === 'heartbeat') return Date.parse(right.last_heartbeat ?? '1970-01-01') - Date.parse(left.last_heartbeat ?? '1970-01-01')
        if (sort === 'cpu') return (right.snapshot?.cpu_percent ?? -1) - (left.snapshot?.cpu_percent ?? -1)
        if (sort === 'disk') return (right.snapshot?.disk_percent ?? -1) - (left.snapshot?.disk_percent ?? -1)
        return left.hostname.localeCompare(right.hostname)
      })
  }, [favorites, query, servers, sort, status])

  function toggleFavorite(serverId: string) {
    const key = `server:${serverId}`
    setFavorites((current) => {
      const next = current.includes(key) ? current.filter((item) => item !== key) : [key, ...current]
      window.localStorage.setItem(FAVORITES_KEY, JSON.stringify(next))
      return next
    })
  }

  async function copy(value: string, key: string) {
    await navigator.clipboard.writeText(value)
    setCopied(key)
    window.setTimeout(() => setCopied((current) => current === key ? null : current), 1200)
  }

  const columns = useMemo<ColumnDef<ServerView>[]>(() => [
    {
      id: 'server',
      header: 'Server',
      cell: ({ row }) => {
        const server = row.original
        const pinned = favorites.includes(`server:${server.server_id}`)
        return <div className="resource-inline-actions"><Tooltip content={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(server.server_id)} aria-label={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><LucideIcon name="star" /></button></Tooltip><div><div className="row-title"><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><Link href={`/infrastructure/servers/${server.server_id}`}>{server.hostname}</Link>{attention(server) ? <span className="badge warning">Attention</span> : null}</div><div className="row-subtitle">{server.snapshot?.os ?? 'Unknown OS'} · <code>{server.server_id.slice(0, 12)}</code> <button className="copy-button" type="button" onClick={() => void copy(server.server_id, server.server_id)}>{copied === server.server_id ? 'Copied' : 'Copy ID'}</button></div></div></div>
      },
    },
    { id: 'status', header: 'Status', cell: ({ row }) => <span className={`state-label ${row.original.online ? 'success' : 'danger'}`}>{row.original.online ? 'Online' : 'Offline'}</span> },
    { id: 'cpu', header: 'CPU', cell: ({ row }) => <Utilization value={row.original.snapshot?.cpu_percent} /> },
    { id: 'memory', header: 'Memory', cell: ({ row }) => <Utilization value={row.original.snapshot?.ram_percent} /> },
    { id: 'disk', header: 'Disk', cell: ({ row }) => <Utilization value={row.original.snapshot?.disk_percent} /> },
    { id: 'services', header: 'Services', cell: ({ row }) => row.original.services.length },
    { id: 'heartbeat', header: 'Heartbeat', cell: ({ row }) => <span title={row.original.last_heartbeat ? new Date(row.original.last_heartbeat).toLocaleString() : undefined}>{relativeTime(row.original.last_heartbeat)}</span> },
  ], [copied, favorites])

  const table = useReactTable({
    data: visible,
    columns,
    getCoreRowModel: getCoreRowModel(),
  })

  return <>
    <div className="stats-grid fleet-summary">
      <div className="stat-card"><div className="stat-label"><span>Servers</span></div><div className="stat-value">{servers.length}</div><div className="stat-meta">registered nodes</div></div>
      <div className="stat-card"><div className="stat-label"><span>Online</span><span className="status-dot online" /></div><div className="stat-value">{online}</div><div className="stat-meta">healthy heartbeats</div></div>
      <div className="stat-card"><div className="stat-label"><span>Attention</span><span className={`status-dot ${needsAttention ? 'warning' : 'online'}`} /></div><div className="stat-value">{needsAttention}</div><div className="stat-meta">actionable nodes</div></div>
      <div className="stat-card"><div className="stat-label"><span>Containers</span></div><div className="stat-value">{containers}</div><div className="stat-meta">visible in snapshots</div></div>
    </div>

    <section className="resource-section server-fleet-section">
      <div className="section-bar resource-toolbar-header">
        <div><h2>Fleet</h2><p>{visible.length} shown · live host telemetry</p></div>
        <div className="resource-toolbar">
          <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search servers…" aria-label="Search servers" />
          <select value={status} onChange={(event) => setStatus(event.target.value as StatusFilter)} aria-label="Filter server status"><option value="all">All statuses</option><option value="online">Online</option><option value="offline">Offline</option><option value="attention">Needs attention</option></select>
          <select value={sort} onChange={(event) => setSort(event.target.value as SortChoice)} aria-label="Sort servers"><option value="name">Name</option><option value="heartbeat">Latest heartbeat</option><option value="cpu">CPU usage</option><option value="disk">Disk usage</option></select>
          <span className={`live-state ${live ? 'online' : 'connecting'}`} title={serverQuery.isError ? 'Live connection unavailable; polling fallback active' : undefined}><span className="status-dot" />{live ? 'Live' : serverQuery.isFetching ? 'Syncing' : 'Polling'}</span>
        </div>
      </div>

      {visible.length === 0 ? <div className="empty-state"><strong>No matching servers</strong>Change the search or status filter to show other nodes.</div> : <>
        <div className="desktop-resource-table table-wrap server-table-wrap">
          <table>
            <thead>{table.getHeaderGroups().map((headerGroup) => <tr key={headerGroup.id}>{headerGroup.headers.map((header) => <th key={header.id}>{header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}</th>)}</tr>)}</thead>
            <tbody>{table.getRowModel().rows.map((row) => <tr key={row.id}>{row.getVisibleCells().map((cell) => <td key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</td>)}</tr>)}</tbody>
          </table>
        </div>

        <ul className="mobile-server-list">
          {visible.map((server) => {
            const pinned = favorites.includes(`server:${server.server_id}`)
            return <li key={server.server_id}>
              <div className="mobile-server-head">
                <Link href={`/infrastructure/servers/${server.server_id}`}><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><strong>{server.hostname}</strong></Link>
                <Tooltip content={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(server.server_id)} aria-label={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><LucideIcon name="star" /></button></Tooltip>
              </div>
              <div className="mobile-server-meta"><span>{server.snapshot?.os ?? 'Unknown OS'}</span><span>{relativeTime(server.last_heartbeat)}</span>{attention(server) ? <span className="state-label warning">Attention</span> : null}</div>
              <div className="mobile-server-metrics"><div><span>CPU</span><strong>{server.snapshot ? `${Math.round(server.snapshot.cpu_percent)}%` : '—'}</strong></div><div><span>Memory</span><strong>{server.snapshot ? `${Math.round(server.snapshot.ram_percent)}%` : '—'}</strong></div><div><span>Disk</span><strong>{server.snapshot ? `${Math.round(server.snapshot.disk_percent)}%` : '—'}</strong></div></div>
            </li>
          })}
        </ul>
      </>}
    </section>
  </>
}
""",
)

resource_css_path = "apps/web/app/resource-polish.css"
resource_css = read(resource_css_path)
if ".argus-tooltip {" not in resource_css:
    resource_css = resource_css.rstrip() + """

.argus-tooltip {
  z-index: 1200;
  max-width: 280px;
  padding: 6px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  background: var(--surface-3);
  color: var(--text-soft);
  font-size: 10px;
  line-height: 1.35;
  box-shadow: 0 8px 24px rgba(0, 0, 0, .3);
  user-select: none;
}

.argus-tooltip-arrow { fill: var(--surface-3); }
"""
    write(resource_css_path, resource_css + "\n")

dev_path = "docs/development.md"
dev = read(dev_path)
dev = dev.replace(
    "CI currently uses pnpm install without a frozen lockfile for the combined web/content typecheck and the committed repository lockfile as the dependency source of truth.",
    "CI installs pnpm dependencies with `--frozen-lockfile`; dependency changes must update and commit `pnpm-lock.yaml`. Rust validation likewise uses the committed `Cargo.lock`.",
    1,
)
web_marker = "These are backend/server variables. Do not expose the Control API credential through public browser environment variables.\n"
if web_marker not in dev:
    raise RuntimeError("missing Web documentation marker")
dev = dev.replace(
    web_marker,
    web_marker
    + """

### Generated Control API contract

Core operator-facing Control API routes publish an OpenAPI document from Rust types. Regenerate the committed browser contract after changing a documented route or schema:

```bash
pnpm generate:api
```

This writes `apps/web/openapi/control-api.json` and `apps/web/lib/generated/control-api.ts`. The generated TypeScript file is owned by this command and should not be edited by hand. The server fleet is the first UI flow using the generated schema together with TanStack Query and TanStack Table; new migrations should follow the same contract-first pattern instead of introducing duplicate handwritten DTOs.
""",
    1,
)
write(dev_path, dev)
