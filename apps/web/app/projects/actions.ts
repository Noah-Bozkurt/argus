'use server'

import { revalidatePath } from 'next/cache'
import { redirect } from 'next/navigation'
import {
  createProject,
  createProjectMilestone,
  createProjectNote,
  createProjectTask,
  linkGitHubRepository,
  syncProjectRepository,
  unlinkProjectRepository,
  updateProjectMilestoneStatus,
  updateProjectNote,
  updateProjectTaskStatus,
  type Milestone,
  type ProjectTask,
  type ProjectSummary,
} from '../../lib/api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function optionalIsoDate(value: string): string | null {
  if (!value) return null
  const parsed = new Date(value)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
}

function csv(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
}

export async function createProjectAction(formData: FormData) {
  const preset = text(formData, 'preset') as ProjectSummary['preset']
  const project = await createProject({
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    preset,
    tags: csv(text(formData, 'tags')),
  })
  redirect(`/projects/${project.id}`)
}

export async function linkRepositoryAction(projectId: string, formData: FormData) {
  await linkGitHubRepository(projectId, text(formData, 'owner'), text(formData, 'name'))
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function syncRepositoryAction(projectId: string, repositoryId: string) {
  await syncProjectRepository(projectId, repositoryId)
  revalidatePath(`/projects/${projectId}`)
}

export async function unlinkRepositoryAction(projectId: string, repositoryId: string) {
  await unlinkProjectRepository(projectId, repositoryId)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function createTaskAction(projectId: string, formData: FormData) {
  const priority = text(formData, 'priority') as ProjectTask['priority']
  const milestoneId = text(formData, 'milestone_id')
  await createProjectTask(projectId, {
    title: text(formData, 'title'),
    description: text(formData, 'description'),
    priority,
    due_at: optionalIsoDate(text(formData, 'due_at')),
    milestone_id: milestoneId || null,
    labels: csv(text(formData, 'labels')),
  })
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function updateTaskStatusAction(
  projectId: string,
  taskId: string,
  formData: FormData,
) {
  const status = text(formData, 'status') as ProjectTask['status']
  await updateProjectTaskStatus(projectId, taskId, status)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function createNoteAction(projectId: string, formData: FormData) {
  await createProjectNote(projectId, text(formData, 'title'), text(formData, 'content'))
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function updateNoteAction(
  projectId: string,
  noteId: string,
  formData: FormData,
) {
  await updateProjectNote(projectId, noteId, text(formData, 'title'), text(formData, 'content'))
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function createMilestoneAction(projectId: string, formData: FormData) {
  await createProjectMilestone(projectId, {
    name: text(formData, 'name'),
    description: text(formData, 'description'),
    due_at: optionalIsoDate(text(formData, 'due_at')),
  })
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}

export async function updateMilestoneStatusAction(
  projectId: string,
  milestoneId: string,
  formData: FormData,
) {
  const status = text(formData, 'status') as Milestone['status']
  await updateProjectMilestoneStatus(projectId, milestoneId, status)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath('/projects')
}
