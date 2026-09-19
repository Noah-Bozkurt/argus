import Link from 'next/link'
import ServiceCatalogSection from '../service-catalog-section'
import EnvironmentsSection from '../environments-section'
import ComposeStacksSection from '../compose-stacks-section'
import DeploymentsReleasesSection from '../deployments-releases-section'
import ReadinessSection from '../readiness-section'
import SitesDomainsSection from '../sites-domains-section'
import SiteMonitoringSection from '../site-monitoring-section'
import MonitorSchedulesSection from '../monitor-schedules-section'
import IncidentAutomationSection from '../incident-automation-section'
import DependencyGraphSection from '../dependency-graph-section'
import IncidentsSection from '../incidents-section'
import StatusPagesSection from '../status-pages-section'
import AddServerSection from '../add-server-section'
import { getProjectEnvironments, getProjectRepositories, getProjectWorkspace } from '../../../../lib/api'
import {
  createMilestoneAction,
  createNoteAction,
  createTaskAction,
  linkRepositoryAction,
  syncRepositoryAction,
  unlinkRepositoryAction,
  updateMilestoneStatusAction,
  updateNoteAction,
  updateTaskStatusAction,
} from '../../actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

function taskStatusClass(status: string): string {
  if (status === 'DONE') return 'success'
  if (status === 'BLOCKED' || status === 'CANCELLED') return 'danger'
  if (status === 'IN_PROGRESS') return 'info'
  return ''
}

export default async function Page({ params }: { params: { projectId: string } }) {
  const { project, tasks, notes, milestones, activity } = await getProjectWorkspace(params.projectId)
  const repositories = await getProjectRepositories(params.projectId)
  return <main>
      <div className="project-section" id="deploy">
        <div className="section-heading">
          <div><span className="eyebrow">Deploy</span><h2>Code & delivery</h2><p>Repositories, compose stacks, services and releases that make up this project.</p></div>
        </div>

        <section className="panel">
          <div className="panel-header"><div><h3>Repositories</h3><p>GitHub remains the source of truth for code, pull requests and issues.</p></div></div>
          <div className="panel-body">
            <details className="create-drawer">
              <summary className="button">+ Link repository</summary>
              <div className="drawer-content">
                <form action={async (formData) => { 'use server'; await linkRepositoryAction(project.id, formData) }}>
                  <div className="form-grid">
                    <label>GitHub owner<input name="owner" required maxLength={100} placeholder="Noah-Bozkurt" /></label>
                    <label>Repository<input name="name" required maxLength={100} placeholder="argus" /></label>
                  </div>
                  <button className="primary" type="submit">Link repository</button>
                </form>
              </div>
            </details>

            {repositories.length === 0 ? <div className="empty-state"><strong>No repositories linked</strong>Connect GitHub code to expose CI and repository metadata.</div> : (
              <ul className="data-list">
                {repositories.map((repository) => (
                  <li className="data-row" key={repository.id}>
                    <div>
                      <div className="row-title"><span className={`status-dot ${repository.sync_status === 'ERROR' ? 'danger' : repository.snapshot.ci.state === 'SUCCESS' ? 'online' : ''}`} /><a href={repository.html_url} target="_blank" rel="noreferrer">{repository.owner}/{repository.name}</a><span className="badge">{repository.visibility}</span></div>
                      <div className="row-subtitle">
                        {repository.default_branch} · {repository.snapshot.open_pull_requests}{repository.snapshot.counts_truncated ? '+' : ''} PRs · {repository.snapshot.open_issues}{repository.snapshot.counts_truncated ? '+' : ''} issues · CI {repository.snapshot.ci.state}
                        {repository.snapshot.latest_commit ? ` · ${repository.snapshot.latest_commit.sha.slice(0, 8)} ${repository.snapshot.latest_commit.message.split('\n')[0]}` : ''}
                      </div>
                      {repository.sync_error ? <div className="text-danger" style={{ marginTop: 5 }}>Sync error: {repository.sync_error}</div> : null}
                    </div>
                    <div className="row-meta">
                      <span>{repository.last_synced_at ? formatDate(repository.last_synced_at) : 'Not synced'}</span>
                      <form action={async () => { 'use server'; await syncRepositoryAction(project.id, repository.id) }}><button className="small" type="submit">Sync</button></form>
                      <form action={async () => { 'use server'; await unlinkRepositoryAction(project.id, repository.id) }}><button className="small danger" type="submit">Unlink</button></form>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>

        <div className="feature-stack stack">
          <ComposeStacksSection projectId={project.id} />
          <ServiceCatalogSection projectId={project.id} />
          <DeploymentsReleasesSection projectId={project.id} />
        </div>
      </div>

  </main>
}
