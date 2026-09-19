 'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
export default function Error({ reset }: { error: Error & { digest?: string }; reset: () => void }) {
 const base = usePathname().split('/content')[0]
 return <section className="empty-state" role="alert"><h2>Content is not ready</h2><p>The workspace may still be preparing, or the content service could not complete this request. Your saved content has not been changed.</p><div className="action-row"><button onClick={reset}>Try again</button><Link href={base}>Back to project</Link></div></section>
}
