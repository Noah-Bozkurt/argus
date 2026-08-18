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

export default async function StatusPagesSection({ projectId }: { projectId: string }) {
  const [pages, inventory, services, incidents] = await Promise.all([
    getStatusPages(projectId),
    getSiteDomainView(projectId),
    getProjectServices(projectId),
    getIncidents(projectId),
  ])

  return (
    <section>
      <h2>Status Pages</h2>
      <p>
        Status pages are internal by default. Publishing a page does not publish any Incident automatically; every public Incident title and message is entered separately.
      </p>

      <h3>Create status page</h3>
      <form action={async (formData) => { 'use server'; await createStatusPageAction(projectId, formData) }}>
        <label>
          Name
          <input name="name" required maxLength={160} placeholder="Argus Status" />
        </label>
        <label>
          Slug
          <input name="slug" required minLength={3} maxLength={80} pattern="[a-zA-Z0-9-]+" placeholder="argus-status" />
        </label>
        <button type="submit">Create internal status page</button>
      </form>

      {pages.length === 0 ? <p>No status pages yet.</p> : pages.map((page) => {
        const existingIncidentIds = new Set(page.incident_publications.map((publication) => publication.incident_id))
        return (
          <article key={page.id}>
            <h3>{page.name} — {page.overall_status}</h3>
            <p>Visibility: {page.visibility} — slug: {page.slug}</p>
            {page.visibility === 'PUBLIC' ? (
              <p><Link href={`/status/${page.slug}`}>Open public status page</Link></p>
            ) : null}

            <form action={async (formData) => { 'use server'; await updateStatusPageAction(projectId, page.id, formData) }}>
              <label>
                Name
                <input name="name" required maxLength={160} defaultValue={page.name} />
              </label>
              <label>
                Slug
                <input name="slug" required minLength={3} maxLength={80} defaultValue={page.slug} />
              </label>
              <label>
                Visibility
                <select name="visibility" defaultValue={page.visibility}>
                  <option value="INTERNAL">Internal</option>
                  <option value="PUBLIC">Public</option>
                </select>
              </label>
              <button type="submit">Save status page</button>
            </form>

            <h4>Components</h4>
            {page.components.length === 0 ? <p>No public components configured.</p> : (
              <ul>
                {page.components.map((component) => (
                  <li key={component.id}>
                    {component.display_name} — {component.public_status}
                    <form action={async () => {
                      'use server'
                      await removeStatusComponentAction(projectId, page.id, component.id)
                    }}>
                      <button type="submit">Remove</button>
                    </form>
                  </li>
                ))}
              </ul>
            )}
            <form action={async (formData) => { 'use server'; await addStatusComponentAction(projectId, page.id, formData) }}>
              <label>
                Resource
                <select name="resource" required defaultValue="">
                  <option value="" disabled>Select Site or Service</option>
                  {inventory.sites.map((site) => (
                    <option key={`SITE-${site.id}`} value={`SITE:${site.id}`}>Site: {site.name}</option>
                  ))}
                  {services.map((service) => (
                    <option key={`SERVICE-${service.id}`} value={`SERVICE:${service.id}`}>Service: {service.name}</option>
                  ))}
                </select>
              </label>
              <label>
                Public display name
                <input name="display_name" required maxLength={160} placeholder="Website" />
              </label>
              <button type="submit">Add public component</button>
            </form>

            <h4>Incident publications</h4>
            {page.incident_publications.length === 0 ? <p>No Incident has been prepared for this page.</p> : (
              page.incident_publications.map((publication) => {
                const incident = incidents.find((item) => item.id === publication.incident_id)
                return (
                  <form key={publication.id} action={async (formData) => {
                    'use server'
                    await updateStatusIncidentPublicationAction(projectId, page.id, formData)
                  }}>
                    <input type="hidden" name="incident_id" value={publication.incident_id} />
                    <p>
                      Internal Incident: {incident?.title ?? publication.incident_id} — {publication.incident_status}
                    </p>
                    <label>
                      Public title
                      <input name="public_title" required maxLength={200} defaultValue={publication.public_title} />
                    </label>
                    <label>
                      Public message
                      <textarea name="public_message" required maxLength={4000} defaultValue={publication.public_message} />
                    </label>
                    <label>
                      <input name="is_published" type="checkbox" defaultChecked={publication.is_published} /> Published
                    </label>
                    <button type="submit">Save publication</button>
                  </form>
                )
              })
            )}

            {incidents.some((incident) => !existingIncidentIds.has(incident.id)) ? (
              <form action={async (formData) => {
                'use server'
                await updateStatusIncidentPublicationAction(projectId, page.id, formData)
              }}>
                <label>
                  Incident
                  <select name="incident_id" required defaultValue="">
                    <option value="" disabled>Select Incident</option>
                    {incidents.filter((incident) => !existingIncidentIds.has(incident.id)).map((incident) => (
                      <option key={incident.id} value={incident.id}>
                        {incident.title} — {incident.status}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Public title
                  <input name="public_title" required maxLength={200} placeholder="Service disruption" />
                </label>
                <label>
                  Public message
                  <textarea name="public_message" required maxLength={4000} placeholder="We are investigating an issue affecting..." />
                </label>
                <label>
                  <input name="is_published" type="checkbox" /> Publish immediately
                </label>
                <button type="submit">Prepare Incident publication</button>
              </form>
            ) : null}

            <form action={async () => { 'use server'; await deleteStatusPageAction(projectId, page.id) }}>
              <button type="submit">Delete status page</button>
            </form>
          </article>
        )
      })}
    </section>
  )
}
