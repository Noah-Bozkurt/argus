'use server'

import { revalidatePath } from 'next/cache'
import { retryDeadJob } from '../../lib/jobs-admin-api'

export async function retryDeadJobAction(jobId: string): Promise<void> {
  await retryDeadJob(jobId)
  revalidatePath('/jobs')
}
