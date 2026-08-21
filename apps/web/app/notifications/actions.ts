'use server'

import { revalidatePath } from 'next/cache'
import {
  acknowledgeNotification,
  createNotificationRule,
  getNotificationInbox,
  markNotificationRead,
  syncNotifications,
  updateNotificationRule,
  type NotificationSeverity,
} from '../../lib/notifications-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function optional(value: string): string | null {
  return value || null
}

export async function createNotificationRuleAction(formData: FormData) {
  await createNotificationRule({
    project_id: optional(text(formData, 'project_id')),
    name: text(formData, 'name'),
    event_pattern: text(formData, 'event_pattern'),
    data_field: optional(text(formData, 'data_field')),
    data_value: optional(text(formData, 'data_value')),
    severity: text(formData, 'severity') as NotificationSeverity,
  })
  revalidatePath('/notifications')
}

export async function updateNotificationRuleAction(ruleId: string, formData: FormData) {
  await updateNotificationRule(ruleId, {
    project_id: optional(text(formData, 'project_id')),
    name: text(formData, 'name'),
    event_pattern: text(formData, 'event_pattern'),
    data_field: optional(text(formData, 'data_field')),
    data_value: optional(text(formData, 'data_value')),
    severity: text(formData, 'severity') as NotificationSeverity,
    enabled: formData.get('enabled') === 'on',
  })
  revalidatePath('/notifications')
}

export async function syncNotificationsAction() {
  await syncNotifications()
  revalidatePath('/notifications')
}

export async function markNotificationReadAction(notificationId: string) {
  await markNotificationRead(notificationId)
  revalidatePath('/notifications')
}

export async function markAllNotificationsReadAction() {
  const inbox = await getNotificationInbox()
  for (const notification of inbox.notifications.filter((item) => !item.read_at)) {
    await markNotificationRead(notification.id)
  }
  revalidatePath('/notifications')
}

export async function acknowledgeNotificationAction(notificationId: string) {
  await acknowledgeNotification(notificationId)
  revalidatePath('/notifications')
}
