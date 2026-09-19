import { getContentWorkspace, getMediaLibrary } from '../../../../../lib/content-api'
import { uploadMediaAction, updateMediaAction, deleteMediaAction } from '../actions'
export default async function Page({ params }: { params: { projectId: string } }) {
 const [workspace, media] = await Promise.all([getContentWorkspace(params.projectId), getMediaLibrary(params.projectId)])
 const canEdit = workspace.permissions.can_edit
 const canManage = workspace.permissions.can_manage
 const publicBase = process.env.ARGUS_CONTENT_PUBLIC_URL?.replace(/\/$/, '')
 return <main><h1>Media</h1>      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Media library</h2><p>Images up to 10 MiB with generated thumbnail, medium and large variants. Public delivery is explicit. Assets included in website releases are retained; upload a new asset to revise them.</p></div><span className="badge">{media.length} assets</span></div>
        <div className="detail-card-body">
          {canEdit ? <details className="create-drawer">
            <summary className="button">+ Upload image</summary>
            <div className="drawer-content">
              <form action={async (formData) => { 'use server'; await uploadMediaAction(params.projectId, formData) }}>
                <div className="form-grid">
                  <label>Image<input name="file" type="file" accept="image/jpeg,image/png,image/webp,image/avif" required /></label>
                  <label>Alternative text<input name="alt" required maxLength={300} /></label>
                  <label className="full">Caption<textarea name="caption" maxLength={2000} /></label>
                  <label className="check-label"><input name="public_read" type="checkbox" value="true" /> Allow public delivery</label>
                </div>
                <button className="primary" type="submit">Upload image</button>
              </form>
            </div>
          </details> : null}

          {media.length === 0 ? <div className="empty-state"><strong>No media</strong>{canEdit ? 'Upload the first image for this project.' : 'No media is configured for this project.'}</div> : (
            <div className="resource-list">
              {media.map((asset) => (
                <article className="resource-card" key={asset.id}>
                  <div className="resource-card-head">
                    <div><h3>{asset.alt}</h3><div className="resource-meta">{asset.filename} · {asset.width ?? '?'}×{asset.height ?? '?'} · {Math.ceil(asset.filesize / 1024)} KiB</div></div>
                    <span className={`badge ${asset.public_read ? 'success' : ''}`}>{asset.public_read ? 'Public' : 'Private'}</span>
                  </div>
                  {asset.caption ? <div className="resource-meta">{asset.caption}</div> : null}
                  <div className="action-row">
                    {asset.public_read && asset.url && publicBase ? <a className="button small" href={`${publicBase}${asset.url}`} target="_blank" rel="noreferrer">View image ↗</a> : null}
                    {canEdit ? <details className="resource-editor">
                      <summary className="button small">Edit asset</summary>
                      <div className="resource-editor-body">
                        <form action={async (formData) => { 'use server'; await updateMediaAction(params.projectId, asset.id, formData) }}>
                          <label>Alternative text<input name="alt" required maxLength={300} defaultValue={asset.alt} /></label>
                          <label>Caption<textarea name="caption" maxLength={2000} defaultValue={asset.caption} /></label>
                          <label className="check-label"><input name="public_read" type="checkbox" defaultChecked={asset.public_read} /> Allow public delivery</label>
                          <button type="submit">Save media details</button>
                        </form>
                        {canManage ? <form action={async (formData) => { 'use server'; await deleteMediaAction(params.projectId, asset.id, formData) }}>
                          <label className="check-label"><input type="checkbox" name="confirm_delete" required /> Confirm permanent deletion of original and variants</label>
                          <button className="danger" type="submit">Delete media</button>
                        </form> : null}
                      </div>
                    </details> : null}
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>
      </section>

</main>
}
