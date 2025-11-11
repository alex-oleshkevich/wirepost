# Mailer

Mailer is a lean CLI utility for delivering SMTP messages. It accepts DSNs or individual host credentials, supports multipart bodies, attachments, custom headers, and provides optional DKIM signing.

## Usage

```bash
MAIL_URL=smtp://user:pass@mail.example.com:587 \
  mail \
  --from ops@example.com \
  --to dev@example.com --cc infra@example.com \
  --subject "Deploy Notice" \
  --text "Plain body" --html "<p>HTML body</p>" \
  --attach ./report.pdf \
  --header "X-Tracking: deploy-42" \
  --print
```

Run `mail --help` for the full flag reference, including templating, retries, verbose logging, and DKIM options.

## Disclaimer

This tool was vibe coded. The author is not responsible for outages, lost emails, or anything that happens when you run it.
