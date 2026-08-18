export type NotificationSeverity = 'INFO' | 'WARNING' | 'CRITICAL'

export type NotificationRule = {
  id: string
  project_id: string | null
  name: string
  event_pattern: string
  data_field: string | null
  data_value: string | null
  severity: NotificationSeverity
  enabled: boolean
  created_at: string
  updated_at: string
}

export type NotificationItem = {
  id: string
  project_id: string
  project_name: string
  title: string
  message: string
  severity: NotificationSeverity
  source_event_type: string
  source_occurred_at: string
  read_at: string | null
  acknowledged_at: string | null
}

export type NotificationInbox = {
  unread_count: number
  unacknowledged_count: number
  notifications: NotificationItem[]
}

export type NotificationSyncResult = {
  scanned_events: number
  enabled_rules: number
  created_notifications: number
  lookback_days: number
}

const controlApi = process.env.ARGUS_CONTROL_API_URL ?? 'http://localhost:8080'

function authHeaders(): Record<string, string> {
  const token = process.env.ARGUS_WEB_API_TOKEN
  const organizationId = process.env.ARGUS_ORG_ID
  const userId = process.env.ARGUS_USER_ID
  if (!token || !organizationId || !userId) {
    throw new Error('ARGUS_WEB_API_TOKEN, ARGUS_ORG_ID and ARGUS_USER_ID are required')
  }
  return {
    authorization: `Bearer ${token}`,
    'x-argus-org-id': organizationId,
    'x-argus-user-id': userId,
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${controlApi}${path}`, {
    ...init,
    cache: 'no-store',
    headers: { ...authHeaders(), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Control API ${response.status}: ${await response.text()}`)
  return response.status === 204 ? (undefined as T) : response.json()
}

export const getNotificationRules = (): Promise<NotificationRule[]> => request('/notification-rules')
export const getNotificationInbox = (): Promise<NotificationInbox> => request('/notifications')

export async function createNotificationRule(input: {
  project_id: string | null
  name: string
  event_pattern: string
  data_field: string | null
  data_value: string | null
  severity: NotificationSeverity
}): Promise<NotificationRule> {
  return request('/notification-rules', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateNotificationRule(
  ruleId: string,
  input: {
    project_id: string | null
    name: string
    event_pattern: string
    data_field: string | null
    data_value: string | null
    severity: NotificationSeverity
    enabled: boolean
  },
): Promise<NotificationRule> {
  return request(`/notification-rules/${ruleId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export const syncNotifications = (): Promise<NotificationSyncResult> =>
  request('/notifications/sync', { method: 'POST' })

export const markNotificationRead = (notificationId: string): Promise<NotificationInbox> =>
  request(`/notifications/${notificationId}/read`, { method: 'POST' })

export const acknowledgeNotification = (notificationId: string): Promise<NotificationInbox> =>
  request(`/notifications/${notificationId}/acknowledge`, { method: 'POST' })
