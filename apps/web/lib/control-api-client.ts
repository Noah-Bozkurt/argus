import type { ControlApiPaths, ControlApiServerView } from './control-api-contract'

type JsonContent<T> = T extends { content: { 'application/json': infer Body } } ? Body : never
type ServersGet = NonNullable<ControlApiPaths['/servers']['get']>
type ServersGetResponses = ServersGet extends { responses: infer Responses } ? Responses : never

export type ControlApiServerList = ServersGetResponses extends { 200: infer Response }
  ? JsonContent<Response>
  : ControlApiServerView[]

async function requestJson<T>(input: RequestInfo | URL, init: RequestInit = {}): Promise<T> {
  const response = await fetch(input, { ...init, cache: 'no-store' })
  if (!response.ok) {
    throw new Error(`Argus API request failed (${response.status})`)
  }
  return response.json() as Promise<T>
}

/**
 * Browser-safe typed access to the Next.js proxy for the generated control API contract.
 * Keep endpoint response types derived from OpenAPI instead of restating Rust DTOs by hand.
 */
export function fetchServers(): Promise<ControlApiServerList> {
  return requestJson<ControlApiServerList>('/api/servers')
}
