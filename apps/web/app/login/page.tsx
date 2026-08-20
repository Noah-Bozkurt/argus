import { loginAction } from './actions'

const errors: Record<string, string> = {
  invalid: 'The email address or password is incorrect.',
  operator_access: 'This account does not have control-panel access.',
  cms_unavailable: 'The CMS is not available right now.',
}

export default function LoginPage({
  searchParams,
}: {
  searchParams: { error?: string; next?: string }
}) {
  const message = searchParams.error ? errors[searchParams.error] : null

  return (
    <main className="login-page">
      <section className="login-card" aria-labelledby="login-title">
        <div className="login-brand" aria-hidden="true">A</div>
        <div className="login-heading">
          <span className="eyebrow">ARGUS CONTROL PLANE</span>
          <h1 id="login-title">Sign in</h1>
          <p>Use your Argus account. Client accounts are taken directly to their CMS workspace.</p>
        </div>

        {message ? <div className="login-error" role="alert">{message}</div> : null}

        <form className="login-form" action={loginAction}>
          <input type="hidden" name="next" value={searchParams.next ?? '/'} />
          <label>
            Email
            <input
              name="email"
              type="email"
              autoComplete="username"
              autoCapitalize="none"
              spellCheck={false}
              required
              autoFocus
            />
          </label>
          <label>
            Password
            <input name="password" type="password" autoComplete="current-password" required />
          </label>
          <button type="submit">Sign in</button>
        </form>

        <p className="login-footnote">Sessions expire automatically and can be revoked by signing out.</p>
      </section>
    </main>
  )
}
