import Link from 'next/link'
import { getProjects } from '../../lib/api'
import { getNotificationInbox, getNotificationRules } from '../../lib/notifications-api'
import {
  acknowledgeNotificationAction,
  createNotificationRuleAction,
  markAllNotificationsReadAction,
  markNotificationReadAction,
  syncNotificationsAction,
  updateNotificationRuleAction,
} from './actions'

function severityClass(severity: string): string {
  if (severity === 'CRITICAL') return 'danger'
  if (severity === 'WARNING') return 'warning'
  return 'info'
}

export default async function NotificationsPage() {
  const [projects, rules, inbox] = await Promise.all([
    getProjects(),
    getNotificationRules(),
    getNotificationInbox(),
  ])

  return (
    <main>
      <div className="page-header compact-page-header">
        <div>
          <h1>Notifications</h1>
          <p>Events that need awareness or acknowledgement across your projects.</p>
        </div>
        <div className="page-actions">
          {inbox.unread_count > 0 ? <form action={markAllNotificationsReadAction}><button type="submit">Mark all read</button></form> : null}
          <form action={syncNotificationsAction}><button className="primary" type="submit">Refresh</button></form>
        </div>
      </div>

      <div className="stats-grid notification-summary">
        <div className="stat-card"><div className="stat-label"><span>Unread</span><span className={`status-dot ${inbox.unread_count ? 'warning' : 'online'}`} /></div><div className="stat-value">{inbox.unread_count}</div><div className="stat-meta">not reviewed</div></div>
        <div className="stat-card"><div className="stat-label"><span>Needs acknowledgement</span><span className={`status-dot ${inbox.unacknowledged_count ? 'danger' : 'online'}`} /></div><div className="stat-value">{inbox.unacknowledged_count}</div><div className="stat-meta">operator action</div></div>
        <div className="stat-card"><div className="stat-label"><span>Rules</span></div><div className="stat-value">{rules.length}</div><div className="stat-meta">{rules.filter((rule) => rule.enabled).length} enabled</div></div>
        <div className="stat-card"><div className="stat-label"><span>Events</span></div><div className="stat-value">{inbox.notifications.length}</div><div className="stat-meta">in this inbox</div></div>
      </div>

      <section className="resource-section notification-inbox">
        <div className="section-bar"><div><h2>Inbox</h2><p>Operational events from the control plane</p></div></div>
        {inbox.notifications.length === 0 ? <div className="empty-state"><strong>No notifications</strong>There are no operational events waiting for you.</div> : (
          <ul className="notification-list">
            {inbox.notifications.map((notification) => (
              <li key={notification.id} className={notification.read_at ? 'read' : 'unread'}>
                <span className={`notification-severity ${severityClass(notification.severity)}`} aria-hidden="true" />
                <div className="notification-content">
                  <div className="notification-title"><strong>{notification.title}</strong><span className={`state-label ${severityClass(notification.severity)}`}>{notification.severity}</span></div>
                  <p>{notification.message}</p>
                  <span>{notification.project_name} · {notification.source_event_type} · {new Date(notification.source_occurred_at).toLocaleString()}</span>
                </div>
                <div className="notification-actions">
                  {!notification.read_at ? <form action={async () => { 'use server'; await markNotificationReadAction(notification.id) }}><button className="small" type="submit">Mark read</button></form> : null}
                  {!notification.acknowledged_at ? <form action={async () => { 'use server'; await acknowledgeNotificationAction(notification.id) }}><button className="small" type="submit">Acknowledge</button></form> : null}
                  <Link className="text-action" href={`/projects/${notification.project_id}`}>Project</Link>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="resource-section notification-rules">
        <div className="section-bar"><div><h2>Rules</h2><p>Choose which event patterns become notifications</p></div></div>
        <div className="panel-body">
          <details className="create-drawer">
            <summary className="button">Create rule</summary>
            <div className="drawer-content">
              <form action={createNotificationRuleAction}>
                <div className="form-grid">
                  <label>Name<input name="name" required maxLength={160} placeholder="Production incident created" /></label>
                  <label>Project scope<select name="project_id" defaultValue=""><option value="">All projects</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
                  <label>Event pattern<input name="event_pattern" required maxLength={120} placeholder="incident.*" /></label>
                  <label>Severity<select name="severity" defaultValue="WARNING"><option value="INFO">Info</option><option value="WARNING">Warning</option><option value="CRITICAL">Critical</option></select></label>
                  <label>Optional data field<input name="data_field" maxLength={120} placeholder="status" /></label>
                  <label>Optional expected value<input name="data_value" maxLength={200} placeholder="DOWN" /></label>
                </div>
                <button className="primary" type="submit">Create rule</button>
              </form>
            </div>
          </details>

          {rules.length === 0 ? <div className="empty-state"><strong>No notification rules yet</strong>Create a rule to turn selected events into notifications.</div> : <div className="rule-list">{rules.map((rule) => (
            <article className="rule-row" key={rule.id}>
              <div className="rule-row-head">
                <div><div className="row-title">{rule.name}<span className={`state-label ${rule.enabled ? 'success' : ''}`}>{rule.enabled ? 'Enabled' : 'Muted'}</span><span className={`state-label ${severityClass(rule.severity)}`}>{rule.severity}</span></div><div className="row-subtitle">{rule.event_pattern} · {rule.project_id ? projects.find((project) => project.id === rule.project_id)?.name ?? rule.project_id : 'All projects'}{rule.data_field ? ` · ${rule.data_field} = ${rule.data_value}` : ''}</div></div>
                <form action={async (formData) => { 'use server'; await updateNotificationRuleAction(rule.id, formData) }}>
                  <input type="hidden" name="name" value={rule.name} /><input type="hidden" name="project_id" value={rule.project_id ?? ''} /><input type="hidden" name="event_pattern" value={rule.event_pattern} /><input type="hidden" name="severity" value={rule.severity} /><input type="hidden" name="data_field" value={rule.data_field ?? ''} /><input type="hidden" name="data_value" value={rule.data_value ?? ''} />{!rule.enabled ? <input type="hidden" name="enabled" value="on" /> : null}
                  <button className="small" type="submit">{rule.enabled ? 'Mute' : 'Enable'}</button>
                </form>
              </div>
              <details className="rule-editor">
                <summary className="button small">Edit</summary>
                <div className="rule-editor-body"><form action={async (formData) => { 'use server'; await updateNotificationRuleAction(rule.id, formData) }}>
                  <div className="form-grid">
                    <label>Name<input name="name" required maxLength={160} defaultValue={rule.name} /></label>
                    <label>Project scope<select name="project_id" defaultValue={rule.project_id ?? ''}><option value="">All projects</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
                    <label>Event pattern<input name="event_pattern" required maxLength={120} defaultValue={rule.event_pattern} /></label>
                    <label>Severity<select name="severity" defaultValue={rule.severity}><option value="INFO">Info</option><option value="WARNING">Warning</option><option value="CRITICAL">Critical</option></select></label>
                    <label>Optional data field<input name="data_field" maxLength={120} defaultValue={rule.data_field ?? ''} /></label>
                    <label>Optional expected value<input name="data_value" maxLength={200} defaultValue={rule.data_value ?? ''} /></label>
                  </div>
                  <label className="checkbox-label"><input name="enabled" type="checkbox" defaultChecked={rule.enabled} /> Enabled</label>
                  <button type="submit">Save rule</button>
                </form></div>
              </details>
            </article>
          ))}</div>}
        </div>
      </section>
    </main>
  )
}
