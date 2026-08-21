import { notFound } from 'next/navigation'
import { getPublicStatusPage } from '../../../lib/status-pages-api'

function statusClass(status: string): string {
  const value = status.toLowerCase()
  if (value.includes('operational') || value.includes('resolved') || value.includes('healthy')) return 'success'
  if (value.includes('major') || value.includes('outage') || value.includes('critical')) return 'danger'
  if (value.includes('degraded') || value.includes('partial') || value.includes('warning')) return 'warning'
  return 'info'
}

export default async function PublicStatusPageRoute({ params }: { params: { slug: string } }) {
  let page
  try {
    page = await getPublicStatusPage(params.slug)
  } catch {
    notFound()
  }

  const overallClass = statusClass(page.overall_status)

  return (
    <main className="public-status-page">
      <header className="status-hero">
        <span className="eyebrow">Argus status</span>
        <h1>{page.name}</h1>
        <p>Last updated {new Date(page.updated_at).toLocaleString()}</p>
        <div className={`overall-status ${overallClass}`}><span className={`status-dot ${overallClass === 'success' ? 'online' : overallClass}`} />{page.overall_status}</div>
      </header>

      <section className="detail-card" style={{ marginBottom: 14 }}>
        <div className="detail-card-header"><div><h2>Components</h2><p>Current availability of public services.</p></div></div>
        {page.components.length === 0 ? <div className="empty-state"><strong>No public components</strong>No components are configured for this status page.</div> : (
          <div>{page.components.map((component) => <div className="component-row" key={component.name}><strong>{component.name}</strong><span className={`badge ${statusClass(component.status)}`}>{component.status}</span></div>)}</div>
        )}
      </section>

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Incidents</h2><p>Published service-impact updates.</p></div></div>
        <div className="detail-card-body">
          {page.incidents.length === 0 ? <div className="callout success">No published incidents.</div> : (
            <ol className="timeline">{page.incidents.map((incident, index) => <li className="timeline-item" key={`${incident.started_at}-${index}`}><div className="timeline-title">{incident.title} <span className={`badge ${statusClass(incident.status)}`}>{incident.status}</span></div><div className="timeline-meta">Started {new Date(incident.started_at).toLocaleString()}{incident.resolved_at ? ` · resolved ${new Date(incident.resolved_at).toLocaleString()}` : ''}</div><div className="timeline-message">{incident.message}</div></li>)}</ol>
          )}
        </div>
      </section>
    </main>
  )
}
