import Link from 'next/link'
import { getProjects } from '../../lib/api'
import { getNotificationInbox, getNotificationRules } from '../../lib/notifications-api'
import {
  acknowledgeNotificationAction,
  createNotificationRuleAction,
  markNotificationReadAction,
  syncNotificationsAction,
  updateNotificationRuleAction,
} from './actions'

export default async function NotificationsPage() {
  const [projects, rules, inbox] = await Promise.all([
    getProjects(),
    getNotificationRules(),
    getNotificationInbox(),
  ])

  return (
    <main>
      <p><Link href="/">← Dashboard</Link></p>
      <h1>Notifications</h1>
      <p>
        {inbox.unread_count} unread — {inbox.unacknowledged_count} unacknowledged. V1 materializes notifications only when you refresh from the event log; there is no background notification worker yet.
      </p>
      <form action={syncNotificationsAction}>
        <button type="submit">Refresh from events</button>
      </form>

      <h2>Inbox</h2>
      {inbox.notifications.length === 0 ? <p>No notifications yet.</p> : (
        <ul>
          {inbox.notifications.map((notification) => (
            <li key={notification.id}>
              <p>
                <strong>{notification.severity} — {notification.title}</strong>
                {' — '}{notification.project_name}
                {' — '}{new Date(notification.source_occurred_at).toLocaleString()}
              </p>
              <p>{notification.message}</p>
              <p>Event: {notification.source_event_type}</p>
              <p>
                {notification.read_at ? `Read ${new Date(notification.read_at).toLocaleString()}` : 'Unread'}
                {' — '}
                {notification.acknowledged_at ? `Acknowledged ${new Date(notification.acknowledged_at).toLocaleString()}` : 'Unacknowledged'}
              </p>
              {!notification.read_at ? (
                <form action={async () => { 'use server'; await markNotificationReadAction(notification.id) }}>
                  <button type="submit">Mark read</button>
                </form>
              ) : null}
              {!notification.acknowledged_at ? (
                <form action={async () => { 'use server'; await acknowledgeNotificationAction(notification.id) }}>
                  <button type="submit">Acknowledge</button>
                </form>
              ) : null}
              <p><Link href={`/projects/${notification.project_id}`}>Open project</Link></p>
            </li>
          ))}
        </ul>
      )}

      <h2>Event rules</h2>
      <p>
        Event patterns are exact (`incident.created`) or suffix-wildcard (`incident.*`). Optional field/value matching can narrow a rule, for example `site.check.completed` with `status = DOWN`.
      </p>

      <h3>Create rule</h3>
      <form action={createNotificationRuleAction}>
        <label>
          Name
          <input name="name" required maxLength={160} placeholder="Production incident created" />
        </label>
        <label>
          Project scope
          <select name="project_id" defaultValue="">
            <option value="">All projects</option>
            {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
          </select>
        </label>
        <label>
          Event pattern
          <input name="event_pattern" required maxLength={120} placeholder="incident.*" />
        </label>
        <label>
          Optional data field
          <input name="data_field" maxLength={120} placeholder="status" />
        </label>
        <label>
          Optional expected value
          <input name="data_value" maxLength={200} placeholder="DOWN" />
        </label>
        <label>
          Severity
          <select name="severity" defaultValue="WARNING">
            <option value="INFO">Info</option>
            <option value="WARNING">Warning</option>
            <option value="CRITICAL">Critical</option>
          </select>
        </label>
        <button type="submit">Create rule</button>
      </form>

      <h3>Rules</h3>
      {rules.length === 0 ? <p>No notification rules yet.</p> : rules.map((rule) => (
        <article key={rule.id}>
          <h4>{rule.name} — {rule.enabled ? 'ENABLED' : 'DISABLED'}</h4>
          <p>
            Pattern {rule.event_pattern}
            {' — '}Severity {rule.severity}
            {' — '}Scope {rule.project_id ? projects.find((project) => project.id === rule.project_id)?.name ?? rule.project_id : 'all projects'}
          </p>
          {rule.data_field ? <p>Filter: {rule.data_field} = {rule.data_value}</p> : null}
          <form action={async (formData) => { 'use server'; await updateNotificationRuleAction(rule.id, formData) }}>
            <label>
              Name
              <input name="name" required maxLength={160} defaultValue={rule.name} />
            </label>
            <label>
              Project scope
              <select name="project_id" defaultValue={rule.project_id ?? ''}>
                <option value="">All projects</option>
                {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
              </select>
            </label>
            <label>
              Event pattern
              <input name="event_pattern" required maxLength={120} defaultValue={rule.event_pattern} />
            </label>
            <label>
              Optional data field
              <input name="data_field" maxLength={120} defaultValue={rule.data_field ?? ''} />
            </label>
            <label>
              Optional expected value
              <input name="data_value" maxLength={200} defaultValue={rule.data_value ?? ''} />
            </label>
            <label>
              Severity
              <select name="severity" defaultValue={rule.severity}>
                <option value="INFO">Info</option>
                <option value="WARNING">Warning</option>
                <option value="CRITICAL">Critical</option>
              </select>
            </label>
            <label>
              <input name="enabled" type="checkbox" defaultChecked={rule.enabled} /> Enabled
            </label>
            <button type="submit">Save rule</button>
          </form>
        </article>
      ))}
    </main>
  )
}
