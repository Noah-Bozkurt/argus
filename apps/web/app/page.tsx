import Link from 'next/link'

export default function DashboardPage() {
  return (
    <main>
      <h1>Argus Dashboard</h1>
      <ul>
        <li><Link href="/projects">Projects</Link></li>
        <li><Link href="/infrastructure/servers">Infrastructure / Servers</Link></li>
      </ul>
    </main>
  )
}
