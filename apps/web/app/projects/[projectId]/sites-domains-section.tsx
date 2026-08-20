import { getProjectEnvironments, getProjectRepositories, getProjectServices } from '../../../lib/api'
import { getProjectDomainLifecycle } from '../../../lib/domain-lifecycle-api'
import { getSiteDomainView } from '../../../lib/sites-domains-api'
import {
  createDomainAction,
  createSiteAction,
  deleteDomainAction,
  deleteSiteAction,
  evaluateDomainLifecycleAction,
  updateDomainAction,
  updateSiteAction,
} from './site-domain-actions'

const routingModes = ['DIRECT', 'CLOUDFLARE_PROXY', 'CLOUDFLARE_TUNNEL'] as const

function dateValue(value: string | null): string {
  return value ? value.slice(0, 10) : ''
}

function statusClass(status: string): string {
  const value = status.toLowerCase()
  if (value.includes('active') || value.includes('healthy') || value.includes('valid') || value.includes('ok')) return 'success'
  if (value.includes('critical') || value.includes('expired') || value.includes('fail')) return 'danger'
  if (value.includes('attention') || value.includes('warning') || value.includes('expiring')) return 'warning'
  return 'info'
}

export default async function SitesDomainsSection({ projectId }: { projectId: string }) {
  const [view, lifecycle, services, environments, repositories] = await Promise.all([
    getSiteDomainView(projectId),
    getProjectDomainLifecycle(projectId),
    getProjectServices(projectId),
    getProjectEnvironments(projectId),
    getProjectRepositories(projectId),
  ])
  const serviceNames = new Map(services.map((service) => [service.id, service.name]))
  const environmentNames = new Map(environments.map((environment) => [environment.id, environment.name]))
  const repositoryNames = new Map(repositories.map((repository) => [repository.id, `${repository.owner}/${repository.name}`]))
  const siteNames = new Map(view.sites.map((site) => [site.id, site.name]))
  const lifecycleByDomain = new Map(lifecycle.map((status) => [status.domain_id, status]))
  const lifecycleProblems = lifecycle.filter((status) => status.overall_status === 'CRITICAL' || status.overall_status === 'ATTENTION')

  return (
    <section>
      <h2>Sites &amp; domains</h2>
      <p>Website inventory, routing and domain lifecycle. Argus observes expiration and TLS state but does not change DNS, renew domains or provision certificates automatically.</p>

      <h3>Sites</h3>
      <details className="create-drawer">
        <summary className="button">+ Add site</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createSiteAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Name<input name="name" required maxLength={160} placeholder="Marketing website" /></label>
              <label>Framework<input name="framework" maxLength={120} placeholder="Astro" /></label>
              <label>Service<select name="service_id" defaultValue=""><option value="">Unlinked</option>{services.map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}</select></label>
              <label>Repository<select name="repository_id" defaultValue=""><option value="">Unlinked / derive from service</option>{repositories.map((repository) => <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>)}</select></label>
              <label>Environment<select name="environment_id" defaultValue=""><option value="">Unassigned / derive from service</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
              <label>Canonical URL<input name="canonical_url" type="url" maxLength={2048} placeholder="https://example.com" /></label>
              <label className="full">Description<textarea name="description" maxLength={8000} /></label>
            </div>
            <button className="primary" type="submit">Add site</button>
          </form>
        </div>
      </details>

      {view.sites.length === 0 ? <div className="empty-state"><strong>No sites</strong>Add a website or web application when this project needs one.</div> : view.sites.map((site) => {
        const linkedDomains = view.domains.filter((domain) => domain.site_id === site.id)
        return (
          <article key={site.id}>
            <div className="resource-card-head">
              <div><h4>{site.name}</h4><div className="resource-meta">{site.description || site.canonical_url || 'No description'}</div></div>
              <div className="action-row"><span className={`badge ${site.lifecycle_status === 'ACTIVE' ? 'success' : site.lifecycle_status === 'PAUSED' ? 'warning' : ''}`}>{site.lifecycle_status}</span><span className={`badge ${statusClass(site.health_status)}`}>{site.health_status}</span></div>
            </div>
            <div className="info-grid" style={{ marginTop: 12 }}>
              <div className="info-item"><span className="info-label">Service</span><span className="info-value">{site.service_id ? serviceNames.get(site.service_id) ?? site.service_id : 'Unlinked'}</span></div>
              <div className="info-item"><span className="info-label">Environment</span><span className="info-value">{site.environment_id ? environmentNames.get(site.environment_id) ?? site.environment_id : 'Unassigned'}</span></div>
              <div className="info-item"><span className="info-label">Repository</span><span className="info-value">{site.repository_id ? repositoryNames.get(site.repository_id) ?? site.repository_id : 'Unlinked'}</span></div>
              <div className="info-item"><span className="info-label">Framework</span><span className="info-value">{site.framework ?? 'Unknown'}</span></div>
            </div>
            <div className="detail-hero-meta">{linkedDomains.map((domain) => <span className="badge" key={domain.id}>{domain.hostname}</span>)}{linkedDomains.length === 0 ? <span className="muted">No domains linked</span> : null}</div>
            {site.canonical_url ? <div className="action-row"><a className="button small" href={site.canonical_url} target="_blank" rel="noreferrer">Open site ↗</a></div> : null}
            <details className="resource-editor">
              <summary className="button small">Edit site</summary>
              <div className="resource-editor-body">
                <form action={async (formData) => { 'use server'; await updateSiteAction(projectId, site.id, formData) }}>
                  <div className="form-grid">
                    <label>Name<input name="name" required maxLength={160} defaultValue={site.name} /></label>
                    <label>Lifecycle<select name="lifecycle_status" defaultValue={site.lifecycle_status}><option value="ACTIVE">Active</option><option value="PAUSED">Paused</option><option value="ARCHIVED">Archived</option></select></label>
                    <label>Service<select name="service_id" defaultValue={site.service_id ?? ''}><option value="">Unlinked</option>{services.map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}</select></label>
                    <label>Repository<select name="repository_id" defaultValue={site.repository_id ?? ''}><option value="">Unlinked / derive from service</option>{repositories.map((repository) => <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>)}</select></label>
                    <label>Environment<select name="environment_id" defaultValue={site.environment_id ?? ''}><option value="">Unassigned / derive from service</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
                    <label>Framework<input name="framework" maxLength={120} defaultValue={site.framework ?? ''} /></label>
                    <label>Canonical URL<input name="canonical_url" type="url" maxLength={2048} defaultValue={site.canonical_url ?? ''} /></label>
                    <label className="full">Description<textarea name="description" maxLength={8000} defaultValue={site.description} /></label>
                  </div>
                  <button type="submit">Save site</button>
                </form>
                <form action={async () => { 'use server'; await deleteSiteAction(projectId, site.id) }}><button className="danger" type="submit" disabled={linkedDomains.length > 0}>Delete site</button></form>
              </div>
            </details>
          </article>
        )
      })}

      <h3>Domains</h3>
      <div className="action-row" style={{ margin: '0 17px 12px' }}>
        <form action={async () => { 'use server'; await evaluateDomainLifecycleAction(projectId) }}><button type="submit">Refresh lifecycle</button></form>
        {lifecycleProblems.length > 0 ? <span className="badge warning">{lifecycleProblems.length} need attention</span> : lifecycle.length > 0 ? <span className="badge success">Lifecycle healthy</span> : null}
      </div>
      <details className="create-drawer">
        <summary className="button">+ Add domain</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createDomainAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Hostname<input name="hostname" required maxLength={253} placeholder="example.com" /></label>
              <label>Site<select name="site_id" defaultValue=""><option value="">Unlinked domain</option>{view.sites.map((site) => <option key={site.id} value={site.id}>{site.name}</option>)}</select></label>
              <label>Registrar<input name="registrar" maxLength={120} placeholder="Cloudflare Registrar" /></label>
              <label>DNS provider<input name="dns_provider" maxLength={120} placeholder="Cloudflare" /></label>
              <label>Routing<select name="routing_mode" defaultValue="DIRECT">{routingModes.map((mode) => <option key={mode} value={mode}>{mode}</option>)}</select></label>
              <label>Expiration<input name="expires_at" type="date" /></label>
              <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="is_primary" type="checkbox" /> Primary domain</label>
            </div>
            <button className="primary" type="submit">Add domain</button>
          </form>
        </div>
      </details>

      {view.domains.length === 0 ? <div className="empty-state"><strong>No domains</strong>Domains linked to project sites will appear here.</div> : view.domains.map((domain) => {
        const domainLifecycle = lifecycleByDomain.get(domain.id)
        return (
          <article key={domain.id}>
            <div className="resource-card-head">
              <div><h4>{domain.hostname}</h4><div className="resource-meta">{domain.site_id ? siteNames.get(domain.site_id) ?? domain.site_id : 'Unlinked'} · {domain.routing_mode}</div></div>
              <div className="action-row">{domain.is_primary ? <span className="badge info">Primary</span> : null}<span className={`badge ${statusClass(domain.tls_status)}`}>TLS {domain.tls_status}</span>{domainLifecycle ? <span className={`badge ${statusClass(domainLifecycle.overall_status)}`}>{domainLifecycle.overall_status}</span> : null}</div>
            </div>
            <div className="info-grid" style={{ marginTop: 12 }}>
              <div className="info-item"><span className="info-label">Registrar</span><span className="info-value">{domain.registrar ?? 'Unknown'}</span></div>
              <div className="info-item"><span className="info-label">DNS provider</span><span className="info-value">{domain.dns_provider ?? 'Unknown'}</span></div>
              <div className="info-item"><span className="info-label">Expiration</span><span className="info-value">{domain.expires_at ? new Date(domain.expires_at).toLocaleDateString() : 'Unknown'}</span></div>
              <div className="info-item"><span className="info-label">Days remaining</span><span className="info-value">{domainLifecycle?.days_until_expiry ?? '—'}</span></div>
            </div>
            {domainLifecycle ? <div className="resource-meta" style={{ marginTop: 10 }}>Expiration {domainLifecycle.expiration_status} · TLS observation {domainLifecycle.tls_status} · evaluated {domainLifecycle.last_evaluated_at ? new Date(domainLifecycle.last_evaluated_at).toLocaleString() : 'never'}</div> : null}
            <details className="resource-editor">
              <summary className="button small">Edit domain</summary>
              <div className="resource-editor-body">
                <form action={async (formData) => { 'use server'; await updateDomainAction(projectId, domain.id, formData) }}>
                  <div className="form-grid">
                    <label>Hostname<input name="hostname" required maxLength={253} defaultValue={domain.hostname} /></label>
                    <label>Site<select name="site_id" defaultValue={domain.site_id ?? ''}><option value="">Unlinked domain</option>{view.sites.map((site) => <option key={site.id} value={site.id}>{site.name}</option>)}</select></label>
                    <label>Registrar<input name="registrar" maxLength={120} defaultValue={domain.registrar ?? ''} /></label>
                    <label>DNS provider<input name="dns_provider" maxLength={120} defaultValue={domain.dns_provider ?? ''} /></label>
                    <label>Routing<select name="routing_mode" defaultValue={domain.routing_mode}>{routingModes.map((mode) => <option key={mode} value={mode}>{mode}</option>)}</select></label>
                    <label>Expiration<input name="expires_at" type="date" defaultValue={dateValue(domain.expires_at)} /></label>
                    <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="is_primary" type="checkbox" defaultChecked={domain.is_primary} /> Primary domain</label>
                  </div>
                  <button type="submit">Save domain</button>
                </form>
                <form action={async () => { 'use server'; await deleteDomainAction(projectId, domain.id) }}><button className="danger" type="submit">Delete domain</button></form>
              </div>
            </details>
          </article>
        )
      })}
    </section>
  )
}
