'use client'

import { useEffect, useMemo, useState } from 'react'
import type { CommandHistoryItem, ServerView } from '../../../../lib/api'

function stateClass(status: string) {
  if (status === 'SUCCEEDED') return 'success'
  if (status === 'FAILED' || status === 'UNKNOWN' || status === 'EXPIRED') return 'danger'
  return 'info'
}

function targetOf(item: CommandHistoryItem): string {
  return item.command.command_type.service ?? item.command.command_type.container ?? item.command.command_type.backup ?? item.command.command_type.profile ?? item.command.command_type.version ?? ''
}

function downloadLog(item: CommandHistoryItem) {
  if (!item.output) return
  const blob = new Blob([item.output], { type: 'text/plain;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `argus-${item.command.command_type.kind.replaceAll('.', '-')}-${item.command.id.slice(0, 8)}.log`
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  URL.revokeObjectURL(url)
}

export default function LiveOperations({ initialServer, initialCommands }: { initialServer: ServerView; initialCommands: CommandHistoryItem[] }) {
  const [server, setServer] = useState(initialServer)
  const [commands, setCommands] = useState(initialCommands)
  const [connection, setConnection] = useState<'live' | 'reconnecting' | 'stale'>('reconnecting')
  const [query, setQuery] = useState('')
  const [failedOnly, setFailedOnly] = useState(false)

  useEffect(() => {
    let lastMessage = Date.now()
    const source = new EventSource(`/api/servers/${initialServer.server_id}/events`)
    source.addEventListener('snapshot', (event) => {
      const data = JSON.parse((event as MessageEvent).data) as { server: ServerView; commands: CommandHistoryItem[] }
      setServer(data.server)
      setCommands(data.commands)
      lastMessage = Date.now()
      setConnection('live')
    })
    source.onerror = () => {
      if (Date.now() - lastMessage > 20_000) setConnection('stale')
    }
    const staleTimer = window.setInterval(() => {
      if (Date.now() - lastMessage > 20_000) setConnection('stale')
    }, 5_000)
    return () => { source.close(); window.clearInterval(staleTimer) }
  }, [initialServer.server_id])

  const visibleCommands = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return commands.filter((item) => {
      if (failedOnly && !['FAILED', 'UNKNOWN', 'EXPIRED'].includes(item.command.status)) return false
      if (!normalized) return true
      return [item.command.command_type.kind, targetOf(item), item.command.status, item.phase ?? '', item.error_code ?? '', item.error_message ?? '', item.output ?? ''].join(' ').toLowerCase().includes(normalized)
    })
  }, [commands, failedOnly, query])

  return <section className="detail-card live-operations" id="activity">
    <div className="detail-card-header"><div><h2>Live operations</h2><p>Utilization and privileged command progress update without refreshing this page.</p></div><span className={`badge ${connection === 'live' ? 'success' : connection === 'stale' ? 'danger' : 'warning'}`}><span className={`status-dot ${connection === 'live' ? 'online' : connection === 'stale' ? 'danger' : 'warning'}`} />{connection}</span></div>
    <div className="detail-card-body">
      <div className="live-metric-strip">
        <div><span>CPU</span><strong>{server.snapshot ? `${server.snapshot.cpu_percent.toFixed(1)}%` : '—'}</strong></div>
        <div><span>Memory</span><strong>{server.snapshot ? `${server.snapshot.ram_percent.toFixed(1)}%` : '—'}</strong></div>
        <div><span>Disk</span><strong>{server.snapshot ? `${server.snapshot.disk_percent.toFixed(1)}%` : '—'}</strong></div>
        <div><span>Agent</span><strong>{server.online ? 'Online' : 'Offline'}</strong></div>
      </div>
      <div className="resource-toolbar operation-filter-bar">
        <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search operations or logs…" aria-label="Search operations and logs" />
        <button className={`resource-chip-filter${failedOnly ? ' active' : ''}`} type="button" onClick={() => setFailedOnly((value) => !value)}>Failures only</button>
      </div>
      {commands.length === 0 ? <div className="empty-state"><strong>No commands yet</strong>Operations will appear here.</div> : visibleCommands.length === 0 ? <div className="empty-state"><strong>No matching operations</strong>Change the search or failure filter.</div> : <ol className="timeline">{visibleCommands.slice(0, 30).map((item) => {
        const target = targetOf(item)
        return <li className="timeline-item operation-item" key={item.command.id}>
          <div className="timeline-title">{item.command.command_type.kind} {target}</div>
          <div className="timeline-meta"><span className={`badge ${stateClass(item.command.status)}`}>{item.command.status}</span>{item.phase ? <span className="operation-phase">{item.phase.toLowerCase()}</span> : null}</div>
          {item.error_code ? <div className="timeline-message text-danger">{item.error_code}: {item.error_message ?? ''}</div> : null}
          {item.output ? <details className="operation-log"><summary className="button small">Full log</summary><div className="operation-log-toolbar"><span>Captured command output</span><div className="resource-inline-actions">{item.output_truncated ? <span className="badge warning">Truncated</span> : <span className="badge success">Complete</span>}<button className="copy-button" type="button" onClick={() => void navigator.clipboard.writeText(item.output ?? '')}>Copy</button><button className="copy-button" type="button" onClick={() => downloadLog(item)}>Download</button></div></div><pre>{item.output}</pre></details> : null}
        </li>
      })}</ol>}
    </div>
  </section>
}
