import { PublicationHistory } from './publication-history'
import { VerifyConnection } from './verify-connection'
import Link from 'next/link'
import { siteRequest, type SiteConnectionView } from '../../../../../lib/content-api'
import { ConnectionForm } from './connection-form'
export default async function Page({ params }: { params: { projectId: string } }) {
 const connection = await siteRequest<SiteConnectionView>(params.projectId)
 const config = `ARGUS_CONTENT_PUBLIC_URL=${process.env.ARGUS_CONTENT_PUBLIC_URL ?? '<your-content-service-url>'}\nARGUS_PROJECT_ID=${params.projectId}`
 return <main><h1>Connect your site</h1><p>Keep your website design in Astro. Manage its content here.</p>
 <section className="panel"><h2>1. Configure Astro</h2><p>Add the Argus Astro integration to your website and configure these build variables.</p><pre>{config}</pre><p>Copy Argus’s packages/astro directory into your website as vendor/argus-astro (source files only). Add the dependency below, install dependencies, and enable the integration. It fetches one fixed release per build and writes the deployment marker.</p><pre>{`// package.json dependencies
"@argus/astro": "file:vendor/argus-astro"

// astro.config.mjs
import argus from '@argus/astro';
export default defineConfig({
  integrations: [argus({ components: yourComponentDefinitions })],
});`}</pre><p>Keep local content mode for the first deployment so Argus can verify the component manifest. After the initial release exists, set <code>ARGUS_CONTENT_MODE=cms</code> in the production build environment. Production builds then read records from <code>virtual:argus-content</code>; they fail if the release cannot load.</p></section>
 <section className="panel"><h2>2. Register your sections</h2><p>Map your Astro components to stable CMS component slugs. Define editable fields in Content structure. Pages select which components they allow.</p><Link className="button" href={`/projects/${params.projectId}/content/structure`}>Configure content structure</Link></section>
 <section className="panel"><h2>3. Connect Cloudflare and preview</h2><p>Deploy the same components to a separate preview Worker. Its preview endpoint renders authorized drafts without changing your public site.</p>{connection.can_connect ? <ConnectionForm projectId={params.projectId} site={connection.site} /> : <p>An organization owner or administrator can connect hosting.</p>}</section>
 <section className="panel"><h2>4. Verify and publish</h2>{connection.site && connection.can_connect && <VerifyConnection projectId={params.projectId} />}<p>Save drafts, preview a page, then publish selected entries together. Cloudflare rebuilds using the saved release. Content is live only after the deployed release is verified.</p>{connection.site && <p>Connected to {connection.site.target} · {connection.site.provider} · {connection.site.branch}</p>}<Link className="button" href={`/projects/${params.projectId}/content/publish`}>Review and publish</Link></section>
 <PublicationHistory projectId={params.projectId} releases={connection.releases} /></main>
}
