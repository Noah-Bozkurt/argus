import Link from 'next/link'
import ServiceCatalogSection from './service-catalog-section'
import EnvironmentsSection from './environments-section'
import ComposeStacksSection from './compose-stacks-section'
import DeploymentsReleasesSection from './deployments-releases-section'
import ReadinessSection from './readiness-section'
import SitesDomainsSection from './sites-domains-section'
import SiteMonitoringSection from './site-monitoring-section'
import MonitorSchedulesSection from './monitor-schedules-section'
import IncidentAutomationSection from './incident-automation-section'
import DependencyGraphSection from './dependency-graph-section'
import IncidentsSection from './incidents-section'
import StatusPagesSection from './status-pages-section'
import { getProjectRepositories, getProjectWorkspace } from '../../../lib/api'
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
} from '../actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

export default async function ProjectPage({ params }: { params: { projectId: string } }) {
  const [workspace, repositories] = await Promise.all([
    getProjectWorkspace(params.projectId),
    getProjectRepositories(params.projectId),
  ])
  const { project, tasks, notes, milestones, activity } = workspace

  return (
    <main>
      <p><Link href="/projects">← Projects</Link></p>
      <h1>{project.name}</h1>
      <p>{project.description || 'No project description.'}</p>
      <p>Preset: {project.preset} — Status: {project.status} — Open tasks: {project.open_tasks}</p>
      <p>Tags: {project.tags.join(', ') || 'none'}</p>
      <p><Link href={`/projects/${project.id}/content`}>Manage content</Link></p>

      <h2>Repositories</h2>
      <p>Link GitHub repositories to this project. Argus reads project-relevant metadata; GitHub remains the source of truth for code, PRs and issues.</p>
      <form action={async (formData) => { 'use server'; await linkRepositoryAction(project.id, formData) }}>
        <label>
          GitHub owner
          <input name="owner" required maxLength={100} placeholder="Noah-Bozkurt" />
        </label>
        <label>
          Repository
          <input name="name" required maxLength={100} placeholder="argus" />
        </label>
        <button type="submit">Link repository</button>
      </form>
      {repositories.length === 0 ? <p>No repositories linked.</p> : (
        <ul>
          {repositories.map((repository) => (
            <li key={repository.id}>
              <p>
                <a href={repository.html_url} target="_blank" rel="noreferrer"><strong>{repository.owner}/{repository.name}</strong></a>
                {' — '}{repository.visibility} — {repository.default_branch} — sync {repository.sync_status}
              </p>
              {repository.sync_error ? <p>Sync error: {repository.sync_error}</p> : null}
              <p>
                PRs: {repository.snapshot.open_pull_requests}{repository.snapshot.counts_truncated ? '+' : ''}
                {' — '}Issues: {repository.snapshot.open_issues}{repository.snapshot.counts_truncated ? '+' : ''}
                {' — '}CI: {repository.snapshot.ci.state} ({repository.snapshot.ci.total_checks} checks)
              </p>
              {repository.snapshot.latest_commit ? (
                <p>
                  Latest commit: <code>{repository.snapshot.latest_commit.sha.slice(0, 12)}</code>
                  {' — '}{repository.snapshot.latest_commit.message.split('\n')[0]}
                  {' — '}{formatDate(repository.snapshot.latest_commit.committed_at)}
                </p>
              ) : <p>No commit metadata available.</p>}
              {repository.snapshot.warnings.length ? (
                <ul>
                  {repository.snapshot.warnings.map((warning) => <li key={warning}>Warning: {warning}</li>)}
                </ul>
              ) : null}
              <p>Last synced: {formatDate(repository.last_synced_at)}</p>
              <form action={async () => { 'use server'; await syncRepositoryAction(project.id, repository.id) }}>
                <button type="submit">Sync now</button>
              </form>
              <form action={async () => { 'use server'; await unlinkRepositoryAction(project.id, repository.id) }}>
                <button type="submit">Unlink</button>
              </form>
            </li>
          ))}
        </ul>
      )}

      <EnvironmentsSection projectId={project.id} />

      <ComposeStacksSection projectId={project.id} />

      <ServiceCatalogSection projectId={project.id} />

      <DeploymentsReleasesSection projectId={project.id} />

      <ReadinessSection projectId={project.id} />

      <SitesDomainsSection projectId={project.id} />

      <SiteMonitoringSection projectId={project.id} />

      <MonitorSchedulesSection projectId={project.id} />

      <IncidentAutomationSection projectId={project.id} />

      <DependencyGraphSection projectId={project.id} />

      <IncidentsSection projectId={project.id} />

      <StatusPagesSection projectId={project.id} />

      <h2>Tasks</h2>
      <form action={async (formData) => { 'use server'; await createTaskAction(project.id, formData) }}>
        <label>
          Title
          <input name="title" required maxLength={200} />
        </label>
        <label>
          Description
          <textarea name="description" maxLength={8000} />
        </label>
        <label>
          Priority
          <select name="priority" defaultValue="MEDIUM">
            <option value="LOW">Low</option>
            <option value="MEDIUM">Medium</option>
            <option value="HIGH">High</option>
            <option value="URGENT">Urgent</option>
          </select>
        </label>
        <label>
          Milestone
          <select name="milestone_id" defaultValue="">
            <option value="">None</option>
            {milestones.filter((milestone) => milestone.status === 'OPEN').map((milestone) => (
              <option key={milestone.id} value={milestone.id}>{milestone.name}</option>
            ))}
          </select>
        </label>
        <label>
          Due
          <input name="due_at" type="datetime-local" />
        </label>
        <label>
          Labels
          <input name="labels" placeholder="backend, launch" />
        </label>
        <button type="submit">Add task</button>
      </form>

      {tasks.length === 0 ? <p>No tasks yet.</p> : (
        <ul>
          {tasks.map((task) => (
            <li key={task.id}>
              <strong>{task.title}</strong> — {task.priority} — due {formatDate(task.due_at)}
              {task.labels.length ? ` — ${task.labels.join(', ')}` : ''}
              {task.description ? <p>{task.description}</p> : null}
              <form action={async (formData) => { 'use server'; await updateTaskStatusAction(project.id, task.id, formData) }}>
                <select name="status" defaultValue={task.status}>
                  <option value="TODO">Todo</option>
                  <option value="IN_PROGRESS">In progress</option>
                  <option value="BLOCKED">Blocked</option>
                  <option value="DONE">Done</option>
                  <option value="CANCELLED">Cancelled</option>
                </select>
                <button type="submit">Update status</button>
              </form>
            </li>
          ))}
        </ul>
      )}

      <h2>Milestones</h2>
      <form action={async (formData) => { 'use server'; await createMilestoneAction(project.id, formData) }}>
        <label>
          Name
          <input name="name" required maxLength={160} />
        </label>
        <label>
          Description
          <textarea name="description" maxLength={4000} />
        </label>
        <label>
          Due
          <input name="due_at" type="datetime-local" />
        </label>
        <button type="submit">Add milestone</button>
      </form>
      {milestones.length === 0 ? <p>No milestones yet.</p> : (
        <ul>
          {milestones.map((milestone) => (
            <li key={milestone.id}>
              <strong>{milestone.name}</strong> — due {formatDate(milestone.due_at)}
              {milestone.description ? <p>{milestone.description}</p> : null}
              <form action={async (formData) => { 'use server'; await updateMilestoneStatusAction(project.id, milestone.id, formData) }}>
                <select name="status" defaultValue={milestone.status}>
                  <option value="OPEN">Open</option>
                  <option value="COMPLETED">Completed</option>
                  <option value="CANCELLED">Cancelled</option>
                </select>
                <button type="submit">Update milestone</button>
              </form>
            </li>
          ))}
        </ul>
      )}

      <h2>Notes</h2>
      <form action={async (formData) => { 'use server'; await createNoteAction(project.id, formData) }}>
        <label>
          Title
          <input name="title" required maxLength={200} />
        </label>
        <label>
          Content
          <textarea name="content" required maxLength={50000} />
        </label>
        <button type="submit">Add note</button>
      </form>
      {notes.length === 0 ? <p>No notes yet.</p> : (
        notes.map((note) => (
          <section key={note.id}>
            <form action={async (formData) => { 'use server'; await updateNoteAction(project.id, note.id, formData) }}>
              <input name="title" defaultValue={note.title} required maxLength={200} />
              <textarea name="content" defaultValue={note.content} required maxLength={50000} />
              <button type="submit">Save note</button>
            </form>
            <small>Updated {formatDate(note.updated_at)}</small>
          </section>
        ))
      )}

      <h2>Activity</h2>
      {activity.length === 0 ? <p>No project activity yet.</p> : (
        <ul>
          {activity.map((item, index) => (
            <li key={`${item.occurred_at}-${index}`}>
              {formatDate(item.occurred_at)} — <strong>{item.event_type}</strong>
              <pre>{JSON.stringify(item.data, null, 2)}</pre>
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}
