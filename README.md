# Wirepost

Wirepost is a lean CLI utility for delivering SMTP messages. It accepts DSNs or individual host credentials, supports multipart bodies, attachments, custom headers, and provides optional DKIM signing.

## Usage

```bash
MAIL_URL=smtp://user:pass@wirepost.example.com:587 \
MAIL_FROM=ops@example.com \
wirepost \
  --to dev@example.com --cc infra@example.com \
  --subject "Deploy {{env}} Notice" \
  --text "Plain body for {{env}}" --html "<p>HTML body</p>" \
  --var env=production \
  --attach ./report.pdf \
  --header "X-Tracking: deploy-42" \
  --header "X-Env: {{env}}" \
  --print
```

`MAIL_URL` and `MAIL_FROM` act as fallbacks for `--dsn` and `--from`, letting you keep secrets out of shell history.

## DKIM

Provide a selector, domain, and private key to have Wirepost sign each message:

```bash
wirepost \
  --dsn smtp://user:pass@wirepost.example.com:587 \
  --from ops@example.com \
  --to dev@example.com \
  --subject "Signed update" \
  --text "Check DKIM headers" \
  --dkim-selector wirepost \
  --dkim-domain example.com \
  --dkim-key ./dkim-private.pem \
  --dkim-algorithm rsa
```

Run `wirepost --help` for the full flag reference, including templating, retries, verbose logging, and DKIM options.

## Disclaimer

This tool was vibe coded. The author is not responsible for outages, lost emails, or anything that happens when you run it.
