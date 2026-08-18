'use server'

import { revalidatePath } from 'next/cache'
import {
  addReleaseComponent,
  createDeployment,
  createRelease,
  updateDeploymentStatus,
  updateReleaseStatus,
  type DeploymentStatus,
  type ReleaseStatus,
} from '../../../lib/deployments-releases-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function optional(value: string): string | null {
  return value || null
}

export async function createDeploymentAction(projectId: string, formData: FormData) {
  await createDeployment(projectId, {
    service_id: text(formData, 'service_id'),
    environment_id: text(formData, 'environment_id'),
    repository_id: optional(text(formData, 'repository_id')),
    source_commit_sha: optional(text(formData, 'source_commit_sha')),
    source_version: optional(text(formData, 'source_version')),
    provider: 'manual',
    notes: text(formData, 'notes'),
    rollback_of_deployment_id: optional(text(formData, 'rollback_of_deployment_id')),
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function updateDeploymentStatusAction(
  projectId: string,
  deploymentId: string,
  formData: FormData,
) {
  await updateDeploymentStatus(
    projectId,
    deploymentId,
    text(formData, 'status') as DeploymentStatus,
    optional(text(formData, 'deployment_url')),
    optional(text(formData, 'error_summary')),
  )
  revalidatePath(`/projects/${projectId}`)
}

export async function createReleaseAction(projectId: string, formData: FormData) {
  await createRelease(projectId, {
    version: text(formData, 'version'),
    name: text(formData, 'name'),
    notes: text(formData, 'notes'),
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function addReleaseComponentAction(
  projectId: string,
  releaseId: string,
  formData: FormData,
) {
  await addReleaseComponent(projectId, releaseId, {
    service_id: text(formData, 'service_id'),
    deployment_id: optional(text(formData, 'deployment_id')),
    version: optional(text(formData, 'version')),
    commit_sha: optional(text(formData, 'commit_sha')),
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function updateReleaseStatusAction(
  projectId: string,
  releaseId: string,
  status: ReleaseStatus,
) {
  await updateReleaseStatus(projectId, releaseId, status)
  revalidatePath(`/projects/${projectId}`)
}
