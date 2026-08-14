# Website deployment

The public Millo website is served by a dedicated unprivileged Debian LXC. It
does not share a process, filesystem, or release lifecycle with the desktop
application.

```text
Internet -> router :80/:443 -> Nginx Proxy Manager -> millo-site:8080
                                                  -> Python static/API server
                                                  -> SQLite feedback database
                                                  -> private LLM moderation API
```

## Public behavior

- `GET /api/health` reports service health without secrets.
- `GET /api/comments` returns approved feedback only.
- `POST /api/comments` accepts comments, bugs, improvements, and wishes.
- LLM failure leaves feedback pending and unpublished.
- Browser output uses DOM text nodes; submitted HTML is never rendered.
- Same-origin requests, body limits, IP-fingerprint rate limiting, a honeypot,
  CSP, and defensive response headers reduce the public attack surface.

The database is stored at `/var/lib/millo-site/feedback.sqlite3`. Back up that
directory before replacing or recreating the container. The application in
`/opt/millo-site` is disposable and can be redeployed from Git.

## Runtime configuration

`/etc/millo-site.env` is root-owned and must not be committed. Required values:

```ini
MILLO_SITE_HOST=0.0.0.0
MILLO_SITE_PORT=8080
MILLO_SITE_DATABASE=/var/lib/millo-site/feedback.sqlite3
MILLO_MODERATION_URL=http://PRIVATE_LLM:8003
MILLO_MODERATION_MODEL=qwen3.6-27b-coding
MILLO_MODERATION_MODE=llama-completion
MILLO_IP_HASH_SECRET=GENERATED_RANDOM_SECRET
```

The Nginx Proxy Manager host forwards `millo-cnc.ru` and `www.millo-cnc.ru` to
the container over HTTP. TLS terminates at NPM. Enable HSTS only after both
names resolve publicly and certificate renewal has been verified.

## Release links

The website points to the immutable GitHub release tag
`v0.1.1-alpha.2`. Each uploaded desktop artifact must have a SHA-256 checksum
in the release notes. Current alpha packages are intentionally described as
unsigned until platform signing and notarization are added.

The website regression suite derives the desktop version, release channel, and
sequence from `package.json`, then checks the DMG, AppImage, and DEB links. A
release metadata change therefore cannot pass `npm run test:site` while the
public download URLs are stale.
