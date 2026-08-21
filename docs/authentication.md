# Argus authentication

Argus uses the Payload `workspace-users` auth collection as the shared human identity provider for the operator web application and the CMS. Machine-to-machine authentication remains separate.

## Human sessions

- The operator UI exposes a first-party `/login` screen instead of relying on reverse-proxy Basic Auth.
- Successful logins create a Payload-backed session and an HTTP-only `payload-token` cookie.
- Production cookies are `Secure`, `SameSite=Lax` and scoped to `ARGUS_AUTH_COOKIE_DOMAIN` so the operator app and the `content.<argus-domain>` CMS can share the session.
- Operator requests are rejected by Next middleware when the session is missing or invalid.
- Logout calls Payload's logout operation before clearing the browser cookie, revoking the active server-side session.
- Sessions expire after eight hours.
- Payload locks an account for ten minutes after five failed login attempts.

## Roles

Workspace roles are deliberately independent from whether a project has a client relationship:

- `owner`: organization owner and full CMS administrator.
- `admin`: organization-wide CMS administrator, except that an admin cannot create or modify owners.
- `member`: membership-scoped user.
- `client`: membership-scoped CMS user that is explicitly denied access to the operator control panel.

Project content permissions remain separate and use `manager`, `editor` and `viewer` memberships. `member` and `client` users can only read project/content resources that are reachable through their explicit project memberships. Client users can only inspect their own membership record.

## Operator boundary

`owner`, `admin` and `member` sessions are eligible for the operator application only when the account has an `argusUserId`. Client accounts normally leave this field empty. The browser never receives the internal `ARGUS_WEB_API_TOKEN`; Web continues to authenticate server-to-server requests to the Control API with that token.

The current Control API client still uses the installation's bootstrap `ARGUS_USER_ID` for control-plane audit attribution. The initial owner maps to that ID. Additional operator identities should not be provisioned until the Web-to-Control API request layer forwards the authenticated user's `argusUserId` per request. CMS access and client isolation do not depend on that follow-up.

## Bootstrap and upgrades

For backwards compatibility, the existing installation credential is reused once as the initial owner's password. The former Basic Auth username remains accepted by the Argus login page as an alias for `ARGUS_OPERATOR_EMAIL`. Caddy no longer performs Basic Auth, so browsers no longer show a native username/password prompt.

The first Payload startup with an empty `workspace-users` collection creates the initial `owner` using `ARGUS_OPERATOR_EMAIL`, `ARGUS_OPERATOR_PASSWORD`, `ARGUS_ORG_ID` and `ARGUS_USER_ID` supplied by the deployment.

## Public routes

`/healthz` and public status pages remain reachable without a human session. The CMS's own public media/content endpoints continue to rely on Payload collection access rules rather than a reverse-proxy password.
