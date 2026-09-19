import SitesDomainsSection from '../sites-domains-section'
export default function Page({ params }: { params: { projectId: string } }) { return <main><h1>Domains</h1><SitesDomainsSection projectId={params.projectId} /></main> }
