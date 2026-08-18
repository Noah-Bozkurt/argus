import Link from 'next/link'
import { getProjectWorkspace } from '../../../lib/api'
import {
  createMilestoneAction,
  createNoteAction,
  createTaskAction,
  updateMilestoneStatusAction,
  updateNoteAction,
  updateTaskStatusAction,
} from '../actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

export default async function ProjectPage({ params }: { params: { projectId: string } }) {
  const workspace = await getProjectWorkspace(params.projectId)
  const { project, tasks, notes, milestones, activity } = workspace

  return (
    <main>
      <p><Link href="/projects">← Projects</Link></p>
      <h1>{project.name}</h1>
      <p>{project.description || 'No project description.'}</p>
      <p>Preset: {project.preset} — Status: {project.status} — Open tasks: {project.open_tasks}</p>
      <p>Tags: {project.tags.join(', ') || 'none'}</p>

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
