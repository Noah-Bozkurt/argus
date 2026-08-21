import { getNotificationInbox } from '../../../../lib/notifications-api'

export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    return Response.json(await getNotificationInbox())
  } catch (error) {
    console.error('Unable to load notification inbox', error)
    return Response.json({ unread_count: 0, unacknowledged_count: 0, notifications: [] }, { status: 503 })
  }
}
