import { notFound } from 'next/navigation'
import { getPublicStatusPage } from '../../../lib/status-pages-api'

export default async function PublicStatusPageRoute({ params }: { params: { slug: string } }) {
  let page
  try {
    page = await getPublicStatusPage(params.slug)
  } catch {
    notFound()
  }

  return (
    <main>
      <h1>{page.name}</h1>
      <p>Overall status: <strong>{page.overall_status}</strong></p>
      <p>Last updated: {new Date(page.updated_at).toLocaleString()}</p>

      <h2>Components</h2>
      {page.components.length === 0 ? <p>No public components configured.</p> : (
        <ul>
          {page.components.map((component) => (
            <li key={component.name}>{component.name} — {component.status}</li>
          ))}
        </ul>
      )}

      <h2>Incidents</h2>
      {page.incidents.length === 0 ? <p>No published incidents.</p> : (
        <article>
          {page.incidents.map((incident, index) => (
            <section key={`${incident.started_at}-${index}`}>
              <h3>{incident.title} — {incident.status}</h3>
              <p>{incident.message}</p>
              <p>Started: {new Date(incident.started_at).toLocaleString()}</p>
              {incident.resolved_at ? <p>Resolved: {new Date(incident.resolved_at).toLocaleString()}</p> : null}
            </section>
          ))}
        </article>
      )}
    </main>
  )
}
