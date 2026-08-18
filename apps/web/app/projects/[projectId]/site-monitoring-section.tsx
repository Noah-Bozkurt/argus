import { getProjectMonitoringView } from '../../../lib/site-monitoring-api'
import { getSiteDomainView } from '../../../lib/sites-domains-api'
import { runSiteMonitorAction, saveSiteMonitorAction } from './site-monitoring-actions'

function defaultTarget(
  site: { id: string; canonical_url: string | null },
  domains: Array<{ site_id: string | null; hostname: string; is_primary: boolean }>,
): string {
  if (site.canonical_url) return site.canonical_url
  const primary = domains.find((domain) => domain.site_id === site.id && domain.is_primary)
  if (primary) return `https://${primary.hostname}/`
  const first = domains.find((domain) => domain.site_id === site.id)
  return first ? `https://${first.hostname}/` : ''
}

export default async function SiteMonitoringSection({ projectId }: { projectId: string }) {
  const [inventory, monitoring] = await Promise.all([
    getSiteDomainView(projectId),
    getProjectMonitoringView(projectId),
  ])
  const monitorBySite = new Map(monitoring.monitors.map((monitor) => [monitor.site_id, monitor]))

  return (
    <section>
      <h2>Site Monitoring</h2>
      <p>
        Checks run only when requested in V1. There is no background scheduler yet. Targets must belong to the Site and private/special IP ranges are blocked before any HTTP request is made.
      </p>
      {inventory.sites.length === 0 ? <p>Add a Site before configuring monitoring.</p> : (
        inventory.sites.map((site) => {
          const monitor = monitorBySite.get(site.id)
          const config = monitor?.config ?? null
          return (
            <article key={site.id}>
              <h3>{site.name} — {site.health_status}</h3>
              <form action={async (formData) => { 'use server'; await saveSiteMonitorAction(projectId, site.id, formData) }}>
                <label>
                  Target URL
                  <input
                    name="target_url"
                    type="url"
                    required
                    maxLength={2048}
                    defaultValue={config?.target_url ?? defaultTarget(site, inventory.domains)}
                    placeholder="https://example.com/"
                  />
                </label>
                <label>
                  Timeout
                  <select name="timeout_seconds" defaultValue={String(config?.timeout_seconds ?? 10)}>
                    <option value="5">5 seconds</option>
                    <option value="10">10 seconds</option>
                    <option value="15">15 seconds</option>
                    <option value="30">30 seconds</option>
                  </select>
                </label>
                <label>
                  <input name="check_robots" type="checkbox" defaultChecked={config?.check_robots ?? true} /> Check /robots.txt
                </label>
                <label>
                  <input name="check_sitemap" type="checkbox" defaultChecked={config?.check_sitemap ?? true} /> Check /sitemap.xml
                </label>
                <button type="submit">Save monitor</button>
              </form>

              {config ? (
                <form action={async () => { 'use server'; await runSiteMonitorAction(projectId, site.id) }}>
                  <button type="submit">Run check now</button>
                </form>
              ) : <p>Save the monitor before running the first check.</p>}

              <h4>Recent checks</h4>
              {!monitor || monitor.checks.length === 0 ? <p>No checks yet.</p> : (
                <ul>
                  {monitor.checks.map((check) => (
                    <li key={check.id}>
                      <p>
                        <strong>{check.overall_status}</strong> — {new Date(check.checked_at).toLocaleString()}
                        {' — '}DNS {check.dns_ok ? 'OK' : 'failed'}
                        {' — '}HTTP {check.http_status ?? '—'}
                        {' — '}TLS {check.tls_status}
                        {' — '}Latency {check.http_latency_ms !== null ? `${check.http_latency_ms} ms` : '—'}
                      </p>
                      <p>
                        robots {check.robots_status ?? '—'} — sitemap {check.sitemap_status ?? '—'}
                        {' — '}resolved {check.resolved_ips.join(', ') || 'none'}
                      </p>
                      {check.error_code ? <p>{check.error_code}: {check.error_message}</p> : null}
                    </li>
                  ))}
                </ul>
              )}
            </article>
          )
        })
      )}
    </section>
  )
}
