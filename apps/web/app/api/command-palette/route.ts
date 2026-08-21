import { getProjects, getServers } from '../../../lib/api'
import { getJobsAdminView } from '../../../lib/jobs-admin-api'

export const dynamic = 'force-dynamic'

type PaletteItem = {
  id: string
  kind: 'project' | 'server' | 'job'
  label: string
  description: string
  href: string
  status?: string
  keywords: string[]
}

export async function GET() {
  const [projectsResult, serversResult, jobsResult] = await Promise.allSettled([
    getProjects(),
    getServers(),
    getJobsAdminView(),
  ])

  const items: PaletteItem[] = []

  if (projectsResult.status === 'fulfilled') {
    for (const project of projectsResult.value) {
      items.push({
        id: `project:${project.id}`,
        kind: 'project',
        label: project.name,
        description: project.description || `${project.preset} project`,
        href: `/projects/${project.id}`,
        status: project.status,
        keywords: [project.preset, project.status, ...project.tags],
      })
    }
  }

  if (serversResult.status === 'fulfilled') {
    for (const server of serversResult.value) {
      items.push({
        id: `server:${server.server_id}`,
        kind: 'server',
        label: server.hostname,
        description: server.online ? 'Managed server · online' : 'Managed server · offline',
        href: `/infrastructure/servers/${server.server_id}`,
        status: server.online ? 'ONLINE' : 'OFFLINE',
        keywords: [server.project_id, server.environment_id, server.snapshot?.os ?? '', server.snapshot?.architecture ?? ''],
      })
    }
  }

  if (jobsResult.status === 'fulfilled') {
    for (const job of jobsResult.value.jobs.slice(0, 100)) {
      items.push({
        id: `job:${job.id}`,
        kind: 'job',
        label: job.job_kind,
        description: `${job.project_name ?? 'Global'} · ${job.resource_key}`,
        href: '/jobs',
        status: job.status,
        keywords: [job.resource_key, job.project_name ?? '', job.status, job.last_error_code ?? ''],
      })
    }
  }

  return Response.json({
    items,
    partial: {
      projects: projectsResult.status === 'rejected',
      servers: serversResult.status === 'rejected',
      jobs: jobsResult.status === 'rejected',
    },
  })
}
