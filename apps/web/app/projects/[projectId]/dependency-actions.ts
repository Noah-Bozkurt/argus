'use server'

import { revalidatePath } from 'next/cache'
import { createDependency, deleteDependency } from '../../../lib/dependency-graph-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function parseNode(value: string): { resourceType: string; resourceId: string } {
  const separator = value.indexOf(':')
  if (separator <= 0 || separator === value.length - 1) {
    throw new Error('Invalid dependency resource')
  }
  return {
    resourceType: value.slice(0, separator),
    resourceId: value.slice(separator + 1),
  }
}

export async function createDependencyAction(projectId: string, formData: FormData) {
  const source = parseNode(text(formData, 'source'))
  const target = parseNode(text(formData, 'target'))
  await createDependency(projectId, {
    source_type: source.resourceType,
    source_id: source.resourceId,
    target_type: target.resourceType,
    target_id: target.resourceId,
    relationship: text(formData, 'relationship') === 'USES' ? 'USES' : 'DEPENDS_ON',
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function deleteDependencyAction(projectId: string, dependencyId: string) {
  await deleteDependency(projectId, dependencyId)
  revalidatePath(`/projects/${projectId}`)
}
