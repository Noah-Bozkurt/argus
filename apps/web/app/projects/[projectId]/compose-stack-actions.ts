'use server'

import { revalidatePath } from 'next/cache'
import {
  createProjectComposeStack,
  deleteProjectComposeStack,
  updateProjectComposeStack,
  type ComposeStack,
} from '../../../lib/compose-stacks-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

export async function createComposeStackAction(projectId: string, formData: FormData) {
  await createProjectComposeStack(projectId, {
    server_id: text(formData, 'server_id'),
    name: text(formData, 'name'),
    compose_project_name: text(formData, 'compose_project_name'),
    description: text(formData, 'description'),
  })
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function updateComposeStackAction(
  projectId: string,
  stackId: string,
  formData: FormData,
) {
  await updateProjectComposeStack(projectId, stackId, {
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    lifecycle_status: text(formData, 'lifecycle_status') as ComposeStack['lifecycle_status'],
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function deleteComposeStackAction(projectId: string, stackId: string) {
  await deleteProjectComposeStack(projectId, stackId)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}
