export type CloudflareTarget = { provider: 'pages' | 'workers'; name: string; branch: string }
export async function cloudflare<T>(token: string, path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`https://api.cloudflare.com/client/v4${path}`, { ...init,
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json', ...init.headers },
    redirect: 'error', signal: AbortSignal.timeout(15000), cache: 'no-store' })
  if (!response.ok) throw new Error('CLOUDFLARE_REQUEST_FAILED')
  const body = await response.json() as { success: boolean; result: T }
  if (!body.success) throw new Error('CLOUDFLARE_REQUEST_FAILED')
  return body.result
}
export function accountPath(account: string): string {
  if (!/^[a-f0-9]{32}$/i.test(account)) throw new Error('INVALID_TARGET')
  return `/accounts/${account}`
}
export function targetName(target: string): string {
  if (!/^[a-z0-9][a-z0-9_-]{0,62}$/i.test(target)) throw new Error('INVALID_TARGET')
  return encodeURIComponent(target)
}
export async function discoverTargets(token: string, account: string): Promise<CloudflareTarget[]> {
  const path = accountPath(account)
  const result: CloudflareTarget[] = []
  // Tokens may deliberately permit only one of the two products.
  const responses = await Promise.allSettled([
    cloudflare<Array<{ name: string; production_branch: string }>>(token, `${path}/pages/projects?per_page=100`),
    cloudflare<Array<{ id: string }>>(token, `${path}/workers/scripts`),
  ])
  if (responses.every(response => response.status === 'rejected')) throw new Error('CLOUDFLARE_REQUEST_FAILED')
  if (responses[0].status === 'fulfilled') for (const item of responses[0].value) result.push({ provider: 'pages', name: item.name, branch: item.production_branch })
  if (responses[1].status === 'fulfilled') for (const item of responses[1].value) result.push({ provider: 'workers', name: item.id, branch: '' })
  return result
}
export async function createHook(token: string, account: string, target: CloudflareTarget, projectId: string): Promise<string> {
  const base = accountPath(account)
  const name = `argus-${projectId}`
  if (!target.branch || target.branch.length > 200) throw new Error('INVALID_TARGET')
  if (target.provider === 'pages') {
    const path = `${base}/pages/projects/${targetName(target.name)}/deploy_hooks`
    const hooks = await cloudflare<Array<{ id: string; name: string; branch: string }>>(token, path)
    const hook = hooks.find(hook => hook.name === name && hook.branch === target.branch) ?? await cloudflare<{ id: string }>(token, path, { method: 'POST', body: JSON.stringify({ name, branch: target.branch }) })
    return `https://api.cloudflare.com/client/v4/pages/webhooks/deploy_hooks/${hook.id}`
  }
  const path = `${base}/builds/workers/${targetName(target.name)}/deploy_hooks`
  const hooks = await cloudflare<Array<{ deploy_hook_uuid: string; deploy_hook_name: string; branch: string }>>(token, path)
  const hook = hooks.find(hook => hook.deploy_hook_name === name && hook.branch === target.branch) ?? await cloudflare<{ deploy_hook_uuid: string }>(token, path, { method: 'POST', body: JSON.stringify({ deploy_hook_name: name, branch: target.branch }) })
  return `https://api.cloudflare.com/client/v4/workers/builds/deploy_hooks/${hook.deploy_hook_uuid}`
}
