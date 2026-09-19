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
  return <main>
      <div className="project-section" id="observe">
        <div className="section-heading">
          <div><span className="eyebrow">Observe</span><h2>Monitoring & incidents</h2><p>Health checks, automation, dependencies, incidents and status communication in one operational view.</p></div>
        </div>
        <div className="feature-stack stack">
          <SiteMonitoringSection projectId={project.id} />
          <MonitorSchedulesSection projectId={project.id} />
          <IncidentAutomationSection projectId={project.id} />
          <DependencyGraphSection projectId={project.id} />
          <IncidentsSection projectId={project.id} />
          <StatusPagesSection projectId={project.id} />
        </div>
      </div>

  </main>
}
