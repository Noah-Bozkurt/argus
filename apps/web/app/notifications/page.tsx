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
      <div className="page-header">
        <div>
          <span className="eyebrow">Operations</span>
          <h1>Notifications</h1>
          <p>Operational events that need awareness or acknowledgement, plus the rules that materialize them.</p>
        </div>
        <div className="page-actions">
          {inbox.unread_count > 0 ? <form action={markAllNotificationsReadAction}><button type="submit">Mark all read</button></form> : null}
          <form action={syncNotificationsAction}><button className="primary" type="submit">Refresh from events</button></form>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-card"><div className="stat-label"><span>Unread</span><span className={`status-dot ${inbox.unread_count ? 'warning' : 'online'}`} /></div><div className="stat-value">{inbox.unread_count}</div><div className="stat-meta">Not reviewed yet</div></div>
        <div className="stat-card"><div className="stat-label"><span>Unacknowledged</span><span className={`status-dot ${inbox.unacknowledged_count ? 'danger' : 'online'}`} /></div><div className="stat-value">{inbox.unacknowledged_count}</div><div className="stat-meta">Needs operator acknowledgement</div></div>
        <div className="stat-card"><div className="stat-label"><span>Rules</span><span className="badge">Configured</span></div><div className="stat-value">{rules.length}</div><div className="stat-meta">{rules.filter((rule) => rule.enabled).length} enabled</div></div>
        <div className="stat-card"><div className="stat-label"><span>Inbox</span><span className="badge info">Events</span></div><div className="stat-value">{inbox.notifications.length}</div><div className="stat-meta">Materialized notifications</div></div>
      </div>

      <section className="panel" style={{ marginBottom: 16 }}>
        <div className="panel-header"><div><h2>Inbox</h2><p>V1 notifications are materialized from the event log when refreshed</p></div></div>
        {inbox.notifications.length === 0 ? <div className="empty-state"><strong>No notifications</strong>There are no materialized operational events right now.</div> : (
          <ul className="data-list">
            {inbox.notifications.map((notification) => (
              <li className="data-row" key={notification.id}>
                <div>
                  <div className="row-title"><span className={`status-dot ${notification.severity === 'CRITICAL' ? 'danger' : notification.severity === 'WARNING' ? 'warning' : ''}`} />{notification.title}<span className={`badge ${severityClass(notification.severity)}`}>{notification.severity}</span></div>
                  <div className="row-subtitle">{notification.project_name} · {notification.source_event_type} · {new Date(notification.source_occurred_at).toLocaleString()}</div>
                  <div style={{ marginTop: 6, color: 'var(--text-soft)', fontSize: 11 }}>{notification.message}</div>
                </div>
                <div className="row-meta">
                  <span>{notification.read_at ? 'Read' : 'Unread'}</span>
                  <span>{notification.acknowledged_at ? 'Acknowledged' : 'Unacknowledged'}</span>
                  {!notification.read_at ? <form action={async () => { 'use server'; await markNotificationReadAction(notification.id) }}><button className="small" type="submit">Mark read</button></form> : null}
                  {!notification.acknowledged_at ? <form action={async () => { 'use server'; await acknowledgeNotificationAction(notification.id) }}><button className="small" type="submit">Acknowledge</button></form> : null}
                  <Link className="panel-link" href={`/projects/${notification.project_id}`}>Project →</Link>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <div className="panel-header"><div><h2>Event rules</h2><p>Exact patterns or suffix wildcards such as incident.*</p></div></div>
        <div className="panel-body">
          <details className="create-drawer">
            <summary className="button">+ Create rule</summary>
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

          {rules.length === 0 ? <div className="empty-state"><strong>No notification rules yet</strong>Create a rule to turn selected events into operator notifications.</div> : <div className="stack">{rules.map((rule) => (
            <article key={rule.id} style={{ padding: 14, border: '1px solid var(--border)', borderRadius: 8, background: '#0e1117' }}>
              <div className="row-title">{rule.name}<span className={`badge ${rule.enabled ? 'success' : ''}`}>{rule.enabled ? 'Enabled' : 'Disabled'}</span><span className={`badge ${severityClass(rule.severity)}`}>{rule.severity}</span></div>
              <div className="row-subtitle" style={{ marginBottom: 12 }}>{rule.event_pattern} · {rule.project_id ? projects.find((project) => project.id === rule.project_id)?.name ?? rule.project_id : 'All projects'}{rule.data_field ? ` · ${rule.data_field} = ${rule.data_value}` : ''}</div>
              <details>
                <summary className="button small">Edit rule</summary>
                <div style={{ marginTop: 12 }}>
                  <form action={async (formData) => { 'use server'; await updateNotificationRuleAction(rule.id, formData) }}>
                    <div className="form-grid">
                      <label>Name<input name="name" required maxLength={160} defaultValue={rule.name} /></label>
                      <label>Project scope<select name="project_id" defaultValue={rule.project_id ?? ''}><option value="">All projects</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
                      <label>Event pattern<input name="event_pattern" required maxLength={120} defaultValue={rule.event_pattern} /></label>
                      <label>Severity<select name="severity" defaultValue={rule.severity}><option value="INFO">Info</option><option value="WARNING">Warning</option><option value="CRITICAL">Critical</option></select></label>
                      <label>Optional data field<input name="data_field" maxLength={120} defaultValue={rule.data_field ?? ''} /></label>
                      <label>Optional expected value<input name="data_value" maxLength={200} defaultValue={rule.data_value ?? ''} /></label>
                    </div>
                    <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center', justifyContent: 'start' }}><input name="enabled" type="checkbox" defaultChecked={rule.enabled} /> Enabled</label>
                    <button type="submit">Save rule</button>
                  </form>
                </div>
              </details>
            </article>
          ))}</div>}
        </div>
      </section>
    </main>
  )
}
