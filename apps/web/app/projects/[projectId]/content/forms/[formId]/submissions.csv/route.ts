import { NextResponse } from 'next/server'

import { downloadFormSubmissionsCsv } from '../../../../../../../lib/content-api'

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

export async function GET(_request: Request, { params }: { params: Promise<{ projectId: string; formId: string }> }) {
  const { projectId, formId } = await params
  if (!UUID.test(projectId) || !UUID.test(formId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  try {
    const exported = await downloadFormSubmissionsCsv(projectId, formId)
    return new NextResponse(exported.body, { headers: { 'content-type': 'text/csv; charset=utf-8', 'content-disposition': exported.disposition,
      'cache-control': 'private, no-store', 'x-content-type-options': 'nosniff' } })
  } catch (error) {
    console.error('Form submissions export failed', { projectId, formId, error })
    return NextResponse.json({ code: 'EXPORT_FAILED' }, { status: 502 })
  }
}
