import {
  getProjectEnvironments,
  getProjectRepositories,
  getProjectServices,
} from '../../../lib/api'
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
      <h2>Sites &amp; Domains</h2>
      <p>
        Sites and domains are inventory records. Cloudflare routing modes describe how traffic reaches the site; V1 does not write DNS records or provision certificates.
      </p>
      <p>
        Domain lifecycle evaluates expiration and recent TLS observations every six hours. It never renews a domain or changes DNS automatically.
      </p>
      <form action={async () => { 'use server'; await evaluateDomainLifecycleAction(projectId) }}>
        <button type="submit">Refresh domain lifecycle</button>
      </form>
      {lifecycleProblems.length > 0 ? (
        <p>{lifecycleProblems.length} domain lifecycle item(s) need attention.</p>
      ) : lifecycle.length > 0 ? (
        <p>No domain lifecycle problems detected.</p>
      ) : null}

      <h3>Add site</h3>
      <form action={async (formData) => { 'use server'; await createSiteAction(projectId, formData) }}>
        <label>
          Name
          <input name="name" required maxLength={160} placeholder="Marketing website" />
        </label>
        <label>
          Description
          <textarea name="description" maxLength={8000} />
        </label>
        <label>
          Service
          <select name="service_id" defaultValue="">
            <option value="">Unlinked</option>
            {services.map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}
          </select>
        </label>
        <label>
          Repository
          <select name="repository_id" defaultValue="">
            <option value="">Unlinked / derive from service</option>
            {repositories.map((repository) => (
              <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>
            ))}
          </select>
        </label>
        <label>
          Environment
          <select name="environment_id" defaultValue="">
            <option value="">Unassigned / derive from service</option>
            {environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}
          </select>
        </label>
        <label>
          Framework
          <input name="framework" maxLength={120} placeholder="Astro" />
        </label>
        <label>
          Canonical URL
          <input name="canonical_url" type="url" maxLength={2048} placeholder="https://example.com" />
        </label>
        <button type="submit">Add site</button>
      </form>

      <h3>Sites</h3>
      {view.sites.length === 0 ? <p>No sites yet.</p> : (
        view.sites.map((site) => {
          const linkedDomains = view.domains.filter((domain) => domain.site_id === site.id)
          return (
            <article key={site.id}>
              <h4>{site.name} — {site.lifecycle_status}</h4>
              <p>
                Health {site.health_status}
                {' — '}Service {site.service_id ? serviceNames.get(site.service_id) ?? site.service_id : 'unlinked'}
                {' — '}Environment {site.environment_id ? environmentNames.get(site.environment_id) ?? site.environment_id : 'unassigned'}
                {' — '}Repository {site.repository_id ? repositoryNames.get(site.repository_id) ?? site.repository_id : 'unlinked'}
              </p>
              {site.framework ? <p>Framework: {site.framework}</p> : null}
              {site.canonical_url ? <p><a href={site.canonical_url} target="_blank" rel="noreferrer">Open site</a></p> : null}
              <p>Domains: {linkedDomains.map((domain) => domain.hostname).join(', ') || 'none'}</p>
              <form action={async (formData) => { 'use server'; await updateSiteAction(projectId, site.id, formData) }}>
                <label>
                  Name
                  <input name="name" required maxLength={160} defaultValue={site.name} />
                </label>
                <label>
                  Description
                  <textarea name="description" maxLength={8000} defaultValue={site.description} />
                </label>
                <label>
                  Service
                  <select name="service_id" defaultValue={site.service_id ?? ''}>
                    <option value="">Unlinked</option>
                    {services.map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}
                  </select>
                </label>
                <label>
                  Repository
                  <select name="repository_id" defaultValue={site.repository_id ?? ''}>
                    <option value="">Unlinked / derive from service</option>
                    {repositories.map((repository) => (
                      <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>
                    ))}
                  </select>
                </label>
                <label>
                  Environment
                  <select name="environment_id" defaultValue={site.environment_id ?? ''}>
                    <option value="">Unassigned / derive from service</option>
                    {environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}
                  </select>
                </label>
                <label>
                  Framework
                  <input name="framework" maxLength={120} defaultValue={site.framework ?? ''} />
                </label>
                <label>
                  Canonical URL
                  <input name="canonical_url" type="url" maxLength={2048} defaultValue={site.canonical_url ?? ''} />
                </label>
                <label>
                  Lifecycle
                  <select name="lifecycle_status" defaultValue={site.lifecycle_status}>
                    <option value="ACTIVE">Active</option>
                    <option value="PAUSED">Paused</option>
                    <option value="ARCHIVED">Archived</option>
                  </select>
                </label>
                <button type="submit">Save site</button>
              </form>
              <form action={async () => { 'use server'; await deleteSiteAction(projectId, site.id) }}>
                <button type="submit" disabled={linkedDomains.length > 0}>Delete site</button>
              </form>
            </article>
          )
        })
      )}

      <h3>Add domain</h3>
      <form action={async (formData) => { 'use server'; await createDomainAction(projectId, formData) }}>
        <label>
          Hostname
          <input name="hostname" required maxLength={253} placeholder="example.com" />
        </label>
        <label>
          Site
          <select name="site_id" defaultValue="">
            <option value="">Unlinked domain</option>
            {view.sites.map((site) => <option key={site.id} value={site.id}>{site.name}</option>)}
          </select>
        </label>
        <label>
          Registrar
          <input name="registrar" maxLength={120} placeholder="Cloudflare Registrar" />
        </label>
        <label>
          DNS provider
          <input name="dns_provider" maxLength={120} placeholder="Cloudflare" />
        </label>
        <label>
          Routing
          <select name="routing_mode" defaultValue="DIRECT">
            {routingModes.map((mode) => <option key={mode} value={mode}>{mode}</option>)}
          </select>
        </label>
        <label>
          Expiration
          <input name="expires_at" type="date" />
        </label>
        <label>
          <input name="is_primary" type="checkbox" /> Primary domain for selected site
        </label>
        <button type="submit">Add domain</button>
      </form>

      <h3>Domains</h3>
      {view.domains.length === 0 ? <p>No domains yet.</p> : (
        view.domains.map((domain) => {
          const domainLifecycle = lifecycleByDomain.get(domain.id)
          return (
            <article key={domain.id}>
              <h4>{domain.hostname}{domain.is_primary ? ' — PRIMARY' : ''}</h4>
              <p>
                Site {domain.site_id ? siteNames.get(domain.site_id) ?? domain.site_id : 'unlinked'}
                {' — '}Routing {domain.routing_mode}
                {' — '}TLS {domain.tls_status}
              </p>
              <p>
                Registrar {domain.registrar ?? 'unknown'} — DNS {domain.dns_provider ?? 'unknown'} — expires {domain.expires_at ? new Date(domain.expires_at).toLocaleDateString() : 'unknown'}
              </p>
              {domainLifecycle ? (
                <p>
                  Lifecycle <strong>{domainLifecycle.overall_status}</strong>
                  {' — '}Expiration {domainLifecycle.expiration_status}
                  {domainLifecycle.days_until_expiry !== null ? ` (${domainLifecycle.days_until_expiry} days)` : ''}
                  {' — '}TLS observation {domainLifecycle.tls_status}
                  {' — '}Last evaluated {domainLifecycle.last_evaluated_at ? new Date(domainLifecycle.last_evaluated_at).toLocaleString() : 'never'}
                </p>
              ) : null}
              <form action={async (formData) => { 'use server'; await updateDomainAction(projectId, domain.id, formData) }}>
                <label>
                  Hostname
                  <input name="hostname" required maxLength={253} defaultValue={domain.hostname} />
                </label>
                <label>
                  Site
                  <select name="site_id" defaultValue={domain.site_id ?? ''}>
                    <option value="">Unlinked domain</option>
                    {view.sites.map((site) => <option key={site.id} value={site.id}>{site.name}</option>)}
                  </select>
                </label>
                <label>
                  Registrar
                  <input name="registrar" maxLength={120} defaultValue={domain.registrar ?? ''} />
                </label>
                <label>
                  DNS provider
                  <input name="dns_provider" maxLength={120} defaultValue={domain.dns_provider ?? ''} />
                </label>
                <label>
                  Routing
                  <select name="routing_mode" defaultValue={domain.routing_mode}>
                    {routingModes.map((mode) => <option key={mode} value={mode}>{mode}</option>)}
                  </select>
                </label>
                <label>
                  Expiration
                  <input name="expires_at" type="date" defaultValue={dateValue(domain.expires_at)} />
                </label>
                <label>
                  <input name="is_primary" type="checkbox" defaultChecked={domain.is_primary} /> Primary domain for selected site
                </label>
                <button type="submit">Save domain</button>
              </form>
              <form action={async () => { 'use server'; await deleteDomainAction(projectId, domain.id) }}>
                <button type="submit">Delete domain</button>
              </form>
            </article>
          )
        })
      )}
    </section>
  )
}
