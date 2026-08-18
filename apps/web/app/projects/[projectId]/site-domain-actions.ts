'use server'

import { revalidatePath } from 'next/cache'
import {
  createDomain,
  createSite,
  deleteDomain,
  deleteSite,
  updateDomain,
  updateSite,
  type DomainRoutingMode,
  type SiteLifecycleStatus,
} from '../../../lib/sites-domains-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function optional(value: string): string | null {
  return value || null
}

export async function createSiteAction(projectId: string, formData: FormData) {
  await createSite(projectId, {
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    service_id: optional(text(formData, 'service_id')),
    repository_id: optional(text(formData, 'repository_id')),
    environment_id: optional(text(formData, 'environment_id')),
    framework: optional(text(formData, 'framework')),
    canonical_url: optional(text(formData, 'canonical_url')),
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function updateSiteAction(projectId: string, siteId: string, formData: FormData) {
  await updateSite(projectId, siteId, {
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    service_id: optional(text(formData, 'service_id')),
    repository_id: optional(text(formData, 'repository_id')),
    environment_id: optional(text(formData, 'environment_id')),
    framework: optional(text(formData, 'framework')),
    canonical_url: optional(text(formData, 'canonical_url')),
    lifecycle_status: text(formData, 'lifecycle_status') as SiteLifecycleStatus,
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function deleteSiteAction(projectId: string, siteId: string) {
  await deleteSite(projectId, siteId)
  revalidatePath(`/projects/${projectId}`)
}

function domainInput(formData: FormData) {
  return {
    site_id: optional(text(formData, 'site_id')),
    hostname: text(formData, 'hostname'),
    registrar: optional(text(formData, 'registrar')),
    dns_provider: optional(text(formData, 'dns_provider')),
    routing_mode: text(formData, 'routing_mode') as DomainRoutingMode,
    is_primary: formData.get('is_primary') === 'on',
    expires_at: optional(text(formData, 'expires_at')),
  }
}

export async function createDomainAction(projectId: string, formData: FormData) {
  await createDomain(projectId, domainInput(formData))
  revalidatePath(`/projects/${projectId}`)
}

export async function updateDomainAction(projectId: string, domainId: string, formData: FormData) {
  await updateDomain(projectId, domainId, domainInput(formData))
  revalidatePath(`/projects/${projectId}`)
}

export async function deleteDomainAction(projectId: string, domainId: string) {
  await deleteDomain(projectId, domainId)
  revalidatePath(`/projects/${projectId}`)
}
