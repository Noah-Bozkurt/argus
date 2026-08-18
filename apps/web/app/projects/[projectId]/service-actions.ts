'use server'

import { revalidatePath } from 'next/cache'
import {
  createCatalogService,
  deleteCatalogService,
  updateCatalogService,
  type CatalogService,
} from '../../../lib/api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function optional(value: string): string | null {
  return value || null
}

function placement(formData: FormData): { environmentId: string | null; serverId: string | null } {
  const serverId = optional(text(formData, 'server_id'))
  // A selected server already owns exactly one environment. Let the backend derive it so
  // stale form state cannot create a server/environment mismatch.
  return {
    environmentId: serverId ? null : optional(text(formData, 'environment_id')),
    serverId,
  }
}

export async function createCatalogServiceAction(projectId: string, formData: FormData) {
  const { environmentId, serverId } = placement(formData)
  await createCatalogService(projectId, {
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    service_type: text(formData, 'service_type'),
    runtime: optional(text(formData, 'runtime')),
    repository_id: optional(text(formData, 'repository_id')),
    environment_id: environmentId,
    server_id: serverId,
    owner_user_id: null,
    endpoint_url: optional(text(formData, 'endpoint_url')),
  })
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function updateCatalogServiceAction(
  projectId: string,
  serviceId: string,
  ownerUserId: string | null,
  formData: FormData,
) {
  const lifecycle = text(formData, 'lifecycle_status') as CatalogService['lifecycle_status']
  const { environmentId, serverId } = placement(formData)
  await updateCatalogService(projectId, serviceId, {
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    service_type: text(formData, 'service_type'),
    runtime: optional(text(formData, 'runtime')),
    repository_id: optional(text(formData, 'repository_id')),
    environment_id: environmentId,
    server_id: serverId,
    owner_user_id: ownerUserId,
    endpoint_url: optional(text(formData, 'endpoint_url')),
    lifecycle_status: lifecycle,
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function deleteCatalogServiceAction(projectId: string, serviceId: string) {
  await deleteCatalogService(projectId, serviceId)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}
