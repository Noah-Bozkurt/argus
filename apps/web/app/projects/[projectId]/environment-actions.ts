'use server'

import { revalidatePath } from 'next/cache'
import {
  createProjectEnvironment,
  deleteProjectEnvironment,
  updateProjectEnvironment,
  type ProjectEnvironment,
} from '../../../lib/api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

export async function createEnvironmentAction(projectId: string, formData: FormData) {
  await createProjectEnvironment(projectId, {
    name: text(formData, 'name'),
    environment_type: text(formData, 'environment_type'),
    description: text(formData, 'description'),
    is_protected: formData.get('is_protected') === 'on',
  })
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function updateEnvironmentAction(
  projectId: string,
  environmentId: string,
  formData: FormData,
) {
  const environmentType = text(formData, 'environment_type') as ProjectEnvironment['environment_type']
  await updateProjectEnvironment(projectId, environmentId, {
    name: text(formData, 'name'),
    environment_type: environmentType,
    description: text(formData, 'description'),
    is_protected: formData.get('is_protected') === 'on',
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function deleteEnvironmentAction(projectId: string, environmentId: string) {
  await deleteProjectEnvironment(projectId, environmentId)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}
