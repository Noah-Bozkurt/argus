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
  const openMilestones = milestones.filter((milestone) => milestone.status === 'OPEN').length
  return <main>
      <div className="project-section" id="work">
        <div className="section-heading">
          <div><span className="eyebrow">Work</span><h2>Tasks, milestones & notes</h2><p>Keep project execution close to the technical context without turning Argus into a client-only system.</p></div>
        </div>

        <div className="split">
          <section className="panel">
            <div className="panel-header"><div><h3>Tasks</h3><p>{project.open_tasks} currently open</p></div></div>
            <div className="panel-body">
              <details className="create-drawer">
                <summary className="button">+ Add task</summary>
                <div className="drawer-content">
                  <form action={async (formData) => { 'use server'; await createTaskAction(project.id, formData) }}>
                    <label>Title<input name="title" required maxLength={200} /></label>
                    <label>Description<textarea name="description" maxLength={8000} /></label>
                    <div className="form-grid">
                      <label>Priority<select name="priority" defaultValue="MEDIUM"><option value="LOW">Low</option><option value="MEDIUM">Medium</option><option value="HIGH">High</option><option value="URGENT">Urgent</option></select></label>
                      <label>Milestone<select name="milestone_id" defaultValue=""><option value="">None</option>{milestones.filter((milestone) => milestone.status === 'OPEN').map((milestone) => <option key={milestone.id} value={milestone.id}>{milestone.name}</option>)}</select></label>
                      <label>Due<input name="due_at" type="datetime-local" /></label>
                      <label>Labels<input name="labels" placeholder="backend, launch" /></label>
                    </div>
                    <button className="primary" type="submit">Add task</button>
                  </form>
                </div>
              </details>
              {tasks.length === 0 ? <div className="empty-state"><strong>No tasks yet</strong>Add work items when this project needs tracking.</div> : (
                <ul className="data-list">
                  {tasks.map((task) => (
                    <li className="data-row" key={task.id}>
                      <div><div className="row-title">{task.title}<span className={`badge ${taskStatusClass(task.status)}`}>{task.status.replaceAll('_', ' ')}</span></div><div className="row-subtitle">{task.priority} priority · due {formatDate(task.due_at)}{task.labels.length ? ` · ${task.labels.join(' · ')}` : ''}</div>{task.description ? <div className="row-subtitle">{task.description}</div> : null}</div>
                      <form action={async (formData) => { 'use server'; await updateTaskStatusAction(project.id, task.id, formData) }}>
                        <select name="status" defaultValue={task.status}><option value="TODO">Todo</option><option value="IN_PROGRESS">In progress</option><option value="BLOCKED">Blocked</option><option value="DONE">Done</option><option value="CANCELLED">Cancelled</option></select>
                        <button className="small" type="submit">Update</button>
                      </form>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </section>

          <section className="panel">
            <div className="panel-header"><div><h3>Milestones</h3><p>{openMilestones} open</p></div></div>
            <div className="panel-body">
              <details className="create-drawer">
                <summary className="button">+ Add milestone</summary>
                <div className="drawer-content">
                  <form action={async (formData) => { 'use server'; await createMilestoneAction(project.id, formData) }}>
                    <label>Name<input name="name" required maxLength={160} /></label>
                    <label>Description<textarea name="description" maxLength={4000} /></label>
                    <label>Due<input name="due_at" type="datetime-local" /></label>
                    <button className="primary" type="submit">Add milestone</button>
                  </form>
                </div>
              </details>
              {milestones.length === 0 ? <div className="empty-state"><strong>No milestones yet</strong>Use milestones for meaningful delivery points.</div> : (
                <ul className="data-list">
                  {milestones.map((milestone) => (
                    <li className="data-row" key={milestone.id}>
                      <div><div className="row-title">{milestone.name}<span className={`badge ${milestone.status === 'COMPLETED' ? 'success' : milestone.status === 'CANCELLED' ? 'danger' : ''}`}>{milestone.status}</span></div><div className="row-subtitle">Due {formatDate(milestone.due_at)}{milestone.description ? ` · ${milestone.description}` : ''}</div></div>
                      <form action={async (formData) => { 'use server'; await updateMilestoneStatusAction(project.id, milestone.id, formData) }}><select name="status" defaultValue={milestone.status}><option value="OPEN">Open</option><option value="COMPLETED">Completed</option><option value="CANCELLED">Cancelled</option></select><button className="small" type="submit">Update</button></form>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </section>
        </div>

        <section className="panel" style={{ marginTop: 14 }}>
          <div className="panel-header"><div><h3>Notes</h3><p>Project knowledge kept next to the work</p></div></div>
          <div className="panel-body">
            <details className="create-drawer">
              <summary className="button">+ Add note</summary>
              <div className="drawer-content"><form action={async (formData) => { 'use server'; await createNoteAction(project.id, formData) }}><label>Title<input name="title" required maxLength={200} /></label><label>Content<textarea name="content" required maxLength={50000} /></label><button className="primary" type="submit">Add note</button></form></div>
            </details>
            {notes.length === 0 ? <div className="empty-state"><strong>No notes yet</strong>Capture decisions and context here.</div> : <div className="stack">{notes.map((note) => <article key={note.id} className="panel-body" style={{ border: '1px solid var(--border)', borderRadius: 8 }}><form action={async (formData) => { 'use server'; await updateNoteAction(project.id, note.id, formData) }}><input name="title" defaultValue={note.title} required maxLength={200} /><textarea name="content" defaultValue={note.content} required maxLength={50000} /><button className="small" type="submit">Save note</button></form><small>Updated {formatDate(note.updated_at)}</small></article>)}</div>}
          </div>
        </section>

        <section className="panel" style={{ marginTop: 14 }}>
          <div className="panel-header"><div><h3>Activity</h3><p>Recent project events</p></div></div>
          {activity.length === 0 ? <div className="empty-state"><strong>No project activity yet</strong>Events appear as the workspace changes.</div> : (
            <ul className="data-list">
              {activity.slice(0, 30).map((item, index) => (
                <li className="data-row" key={`${item.occurred_at}-${index}`}>
                  <div><div className="row-title">{item.event_type}</div><div className="row-subtitle">{formatDate(item.occurred_at)}</div></div>
                  <details><summary className="button small">Details</summary><pre>{JSON.stringify(item.data, null, 2)}</pre></details>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
  </main>
}
