import Link from 'next/link'
import { getIncidents } from '../../../lib/incidents-api'
import { getProjectServices } from '../../../lib/api'
import { getSiteDomainView } from '../../../lib/sites-domains-api'
import { getStatusPages } from '../../../lib/status-pages-api'
import {
  addStatusComponentAction,
  createStatusPageAction,
  deleteStatusPageAction,
  removeStatusComponentAction,
  updateStatusIncidentPublicationAction,
  updateStatusPageAction,
} from './status-page-actions'

function statusClass(status: string): string {
  const value = status.toLowerCase()
  if (value.includes('operational') || value.includes('healthy')) return 'success'
  if (value.includes('critical') || value.includes('major')) return 'danger'
  if (value.includes('attention') || value.includes('degraded')) return 'warning'
  return 'info'
}

export default async function StatusPagesSection({ projectId }: { projectId: string }) {
  const [pages, inventory, services, incidents] = await Promise.all([
    getStatusPages(projectId),
    getSiteDomainView(projectId),
    getProjectServices(projectId),
    getIncidents(projectId),
  ])

  return (
    <section>
      <h2>Status pages</h2>
      <p>Status pages stay internal until explicitly published. Incident publication is also explicit: public titles and messages are separate from internal incident details.</p>

      <h3>Status pages</h3>
      <details className="create-drawer">
        <summary className="button">+ Create status page</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createStatusPageAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Name<input name="name" required maxLength={160} placeholder="Argus Status" /></label>
              <label>Slug<input name="slug" required minLength={3} maxLength={80} pattern="[a-zA-Z0-9-]+" placeholder="argus-status" /></label>
            </div>
            <button className="primary" type="submit">Create internal status page</button>
          </form>
        </div>
      </details>

      {pages.length === 0 ? <div className="empty-state"><strong>No status pages</strong>Create a status surface when this project needs public communication.</div> : pages.map((page) => {
        const existingIncidentIds = new Set(page.incident_publications.map((publication) => publication.incident_id))
        const unpublishedIncidents = incidents.filter((incident) => !existingIncidentIds.has(incident.id))
        return (
          <article key={page.id}>
            <div className="resource-card-head">
              <div><h4>{page.name}</h4><div className="resource-meta">/{page.slug} · {page.components.length} components · {page.incident_publications.length} incident publications</div></div>
              <div className="action-row"><span className={`badge ${page.visibility === 'PUBLIC' ? 'success' : ''}`}>{page.visibility}</span><span className={`badge ${statusClass(page.overall_status)}`}>{page.overall_status}</span></div>
            </div>
            {page.visibility === 'PUBLIC' ? <div className="action-row"><Link className="button small" href={`/status/${page.slug}`}>Open public page ↗</Link></div> : null}

            <div className="info-grid" style={{ marginTop: 12 }}>
              <div className="info-item"><span className="info-label">Components</span><span className="info-value">{page.components.length}</span></div>
              <div className="info-item"><span className="info-label">Published incidents</span><span className="info-value">{page.incident_publications.filter((publication) => publication.is_published).length}</span></div>
            </div>

            {page.components.length > 0 ? <div className="resource-list" style={{ marginTop: 12 }}>{page.components.map((component) => <div className="resource-card" key={component.id}><div className="resource-card-head"><strong>{component.display_name}</strong><span className={`badge ${statusClass(component.public_status)}`}>{component.public_status}</span></div><div className="action-row"><form action={async () => { 'use server'; await removeStatusComponentAction(projectId, page.id, component.id) }}><button className="small danger" type="submit">Remove</button></form></div></div>)}</div> : null}

            <details className="resource-editor">
              <summary className="button small">Configure status page</summary>
              <div className="resource-editor-body">
                <form action={async (formData) => { 'use server'; await updateStatusPageAction(projectId, page.id, formData) }}>
                  <div className="form-grid">
                    <label>Name<input name="name" required maxLength={160} defaultValue={page.name} /></label>
                    <label>Slug<input name="slug" required minLength={3} maxLength={80} defaultValue={page.slug} /></label>
                    <label>Visibility<select name="visibility" defaultValue={page.visibility}><option value="INTERNAL">Internal</option><option value="PUBLIC">Public</option></select></label>
                  </div>
                  <button type="submit">Save status page</button>
                </form>

                <form action={async (formData) => { 'use server'; await addStatusComponentAction(projectId, page.id, formData) }}>
                  <strong>Add public component</strong>
                  <div className="form-grid">
                    <label>Resource<select name="resource" required defaultValue=""><option value="" disabled>Select Site or Service</option>{inventory.sites.map((site) => <option key={`SITE-${site.id}`} value={`SITE:${site.id}`}>Site: {site.name}</option>)}{services.map((service) => <option key={`SERVICE-${service.id}`} value={`SERVICE:${service.id}`}>Service: {service.name}</option>)}</select></label>
                    <label>Public display name<input name="display_name" required maxLength={160} placeholder="Website" /></label>
                  </div>
                  <button type="submit">Add component</button>
                </form>

                {page.incident_publications.map((publication) => {
                  const incident = incidents.find((item) => item.id === publication.incident_id)
                  return (
                    <form key={publication.id} action={async (formData) => { 'use server'; await updateStatusIncidentPublicationAction(projectId, page.id, formData) }}>
                      <input type="hidden" name="incident_id" value={publication.incident_id} />
                      <strong>{incident?.title ?? publication.incident_id}</strong>
                      <div className="resource-meta">Internal status: {publication.incident_status}</div>
                      <div className="form-grid">
                        <label>Public title<input name="public_title" required maxLength={200} defaultValue={publication.public_title} /></label>
                        <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="is_published" type="checkbox" defaultChecked={publication.is_published} /> Published</label>
                        <label className="full">Public message<textarea name="public_message" required maxLength={4000} defaultValue={publication.public_message} /></label>
                      </div>
                      <button type="submit">Save publication</button>
                    </form>
                  )
                })}

                {unpublishedIncidents.length > 0 ? (
                  <form action={async (formData) => { 'use server'; await updateStatusIncidentPublicationAction(projectId, page.id, formData) }}>
                    <strong>Prepare incident publication</strong>
                    <div className="form-grid">
                      <label>Incident<select name="incident_id" required defaultValue=""><option value="" disabled>Select Incident</option>{unpublishedIncidents.map((incident) => <option key={incident.id} value={incident.id}>{incident.title} — {incident.status}</option>)}</select></label>
                      <label>Public title<input name="public_title" required maxLength={200} placeholder="Service disruption" /></label>
                      <label className="full">Public message<textarea name="public_message" required maxLength={4000} placeholder="We are investigating an issue affecting..." /></label>
                      <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="is_published" type="checkbox" /> Publish immediately</label>
                    </div>
                    <button type="submit">Prepare publication</button>
                  </form>
                ) : null}

                <form action={async () => { 'use server'; await deleteStatusPageAction(projectId, page.id) }}><button className="danger" type="submit">Delete status page</button></form>
              </div>
            </details>
          </article>
        )
      })}
    </section>
  )
}
