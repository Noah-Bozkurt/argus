import { getProjectMonitoringView } from '../../../lib/site-monitoring-api'
import { getSiteDomainView } from '../../../lib/sites-domains-api'
import { runSiteMonitorAction, saveSiteMonitorAction } from './site-monitoring-actions'

function defaultTarget(site: { id: string; canonical_url: string | null }, domains: Array<{ site_id: string | null; hostname: string; is_primary: boolean }>): string {
  if (site.canonical_url) return site.canonical_url
  const primary = domains.find((domain) => domain.site_id === site.id && domain.is_primary)
  if (primary) return `https://${primary.hostname}/`
  const first = domains.find((domain) => domain.site_id === site.id)
  return first ? `https://${first.hostname}/` : ''
}

function statusClass(status: string): string {
  const value = status.toLowerCase()
  if (value.includes('down') || value.includes('error') || value.includes('fail')) return 'danger'
  if (value.includes('degraded') || value.includes('warning')) return 'warning'
  if (value.includes('up') || value.includes('healthy') || value.includes('ok') || value.includes('valid')) return 'success'
  return 'info'
}

export default async function SiteMonitoringSection({ projectId }: { projectId: string }) {
  const [inventory, monitoring] = await Promise.all([getSiteDomainView(projectId), getProjectMonitoringView(projectId)])
  const monitorBySite = new Map(monitoring.monitors.map((monitor) => [monitor.site_id, monitor]))

  return (
    <section>
      <h2>Site monitoring</h2>
      <p>SSRF-safe HTTP health checks for project sites. Targets must belong to the site and private/special IP ranges are blocked before requests are made.</p>

      <h3>Monitors</h3>
      {inventory.sites.length === 0 ? <div className="empty-state"><strong>No sites to monitor</strong>Add a site before configuring monitoring.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {inventory.sites.map((site) => {
            const monitor = monitorBySite.get(site.id)
            const config = monitor?.config ?? null
            const latest = monitor?.checks[0] ?? null
            const target = config?.target_url ?? defaultTarget(site, inventory.domains)
            return (
              <article className="resource-card" key={site.id}>
                <div className="resource-card-head">
                  <div><h4>{site.name}</h4><div className="resource-meta">{target || 'No target configured'}</div></div>
                  <span className={`badge ${statusClass(latest?.overall_status ?? site.health_status)}`}>{latest?.overall_status ?? site.health_status}</span>
                </div>

                {latest ? (
                  <div className="status-grid" style={{ marginTop: 12 }}>
                    <div className="status-item"><span className="info-label">DNS</span><span className="info-value"><span className={`badge ${latest.dns_ok ? 'success' : 'danger'}`}>{latest.dns_ok ? 'OK' : 'Failed'}</span></span></div>
                    <div className="status-item"><span className="info-label">HTTP</span><span className="info-value">{latest.http_status ?? '—'}</span></div>
                    <div className="status-item"><span className="info-label">TLS</span><span className="info-value"><span className={`badge ${statusClass(latest.tls_status)}`}>{latest.tls_status}</span></span></div>
                    <div className="status-item"><span className="info-label">Latency</span><span className="info-value">{latest.http_latency_ms !== null ? `${latest.http_latency_ms} ms` : '—'}</span></div>
                  </div>
                ) : <div className="callout" style={{ marginTop: 12 }}>No checks have run yet.</div>}

                <div className="action-row">
                  {config ? <form action={async () => { 'use server'; await runSiteMonitorAction(projectId, site.id) }}><button type="submit">Run check</button></form> : null}
                  <details className="resource-editor">
                    <summary className="button small">{config ? 'Configure monitor' : 'Set up monitor'}</summary>
                    <div className="resource-editor-body">
                      <form action={async (formData) => { 'use server'; await saveSiteMonitorAction(projectId, site.id, formData) }}>
                        <div className="form-grid">
                          <label className="full">Target URL<input name="target_url" type="url" required maxLength={2048} defaultValue={target} placeholder="https://example.com/" /></label>
                          <label>Timeout<select name="timeout_seconds" defaultValue={String(config?.timeout_seconds ?? 10)}><option value="5">5 seconds</option><option value="10">10 seconds</option><option value="15">15 seconds</option><option value="30">30 seconds</option></select></label>
                          <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="check_robots" type="checkbox" defaultChecked={config?.check_robots ?? true} /> Check /robots.txt</label>
                          <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="check_sitemap" type="checkbox" defaultChecked={config?.check_sitemap ?? true} /> Check /sitemap.xml</label>
                        </div>
                        <button type="submit">Save monitor</button>
                      </form>
                    </div>
                  </details>
                </div>

                {monitor && monitor.checks.length > 0 ? (
                  <details className="log-details" style={{ marginTop: 12 }}>
                    <summary>Recent checks · {monitor.checks.length}</summary>
                    <div className="resource-list" style={{ padding: 10 }}>
                      {monitor.checks.map((check) => (
                        <div className="resource-card" key={check.id}>
                          <div className="resource-card-head"><div><strong>{new Date(check.checked_at).toLocaleString()}</strong><div className="resource-meta">robots {check.robots_status ?? '—'} · sitemap {check.sitemap_status ?? '—'} · {check.resolved_ips.join(', ') || 'no resolved IPs'}</div></div><span className={`badge ${statusClass(check.overall_status)}`}>{check.overall_status}</span></div>
                          {check.error_code ? <div className="callout danger" style={{ marginTop: 8 }}>{check.error_code}: {check.error_message}</div> : null}
                        </div>
                      ))}
                    </div>
                  </details>
                ) : null}
              </article>
            )
          })}
        </div>
      )}
    </section>
  )
}
