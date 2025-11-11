use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use clap::{ArgAction, Parser};
use lettre::{
    SmtpTransport, Transport,
    message::{
        Attachment, Mailbox, Message, MultiPart, SinglePart,
        header::{ContentType, HeaderName, HeaderValue},
    },
    transport::smtp::authentication::Credentials,
};
use mime_guess::mime;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "mail", about = "Send an email via SMTP", version)]
struct Args {
    /// SMTP DSN, e.g. smtp://user:pass@example.com:587
    #[arg(long)]
    dsn: Option<String>,
    /// SMTP host (used when DSN is not supplied)
    #[arg(long)]
    host: Option<String>,
    /// SMTP port (defaults to 587)
    #[arg(long)]
    port: Option<u16>,
    /// SMTP username (used when DSN is not supplied)
    #[arg(long)]
    user: Option<String>,
    /// SMTP password (used when DSN is not supplied)
    #[arg(long)]
    pass: Option<String>,
    /// Sender mailbox
    #[arg(long, required = true)]
    from: String,
    /// Primary recipients (repeatable)
    #[arg(long = "to", action = ArgAction::Append, required = true)]
    to: Vec<String>,
    /// CC recipients (repeatable)
    #[arg(long = "cc", action = ArgAction::Append)]
    cc: Vec<String>,
    /// BCC recipients (repeatable)
    #[arg(long = "bcc", action = ArgAction::Append)]
    bcc: Vec<String>,
    /// Subject line
    #[arg(long, default_value = "")]
    subject: String,
    /// Plain-text body
    #[arg(long)]
    text: Option<String>,
    /// HTML body
    #[arg(long)]
    html: Option<String>,
    /// File attachments (repeatable)
    #[arg(long = "attach", action = ArgAction::Append)]
    attachments: Vec<PathBuf>,
    /// Print the fully formatted message instead of (or in addition to) sending
    #[arg(long)]
    print: bool,
    /// Additional headers in the form `Name: Value` (repeatable)
    #[arg(long = "header", action = ArgAction::Append)]
    headers: Vec<String>,
}

struct Connection {
    host: String,
    port: u16,
    auth: Option<Auth>,
    security: TransportSecurity,
}

struct Auth {
    user: String,
    pass: String,
}

enum TransportSecurity {
    Plain,
    TlsWrapper,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let conn = resolve_connection(&args)?;
    let message = build_message(&args)?;

    if args.print {
        let rendered = message.formatted();
        println!("{}", String::from_utf8_lossy(&rendered));
    }

    let mut builder = match conn.security {
        TransportSecurity::TlsWrapper => SmtpTransport::relay(&conn.host)
            .with_context(|| format!("failed to configure relay for {}", conn.host))?
            .port(conn.port),
        TransportSecurity::Plain => SmtpTransport::builder_dangerous(&conn.host).port(conn.port),
    };
    if let Some(auth) = &conn.auth {
        builder = builder.credentials(Credentials::new(auth.user.clone(), auth.pass.clone()));
    }
    let mailer = builder.build();

    mailer
        .send(&message)
        .context("failed to send message via SMTP")?;

    println!("Email sent");
    Ok(())
}

fn resolve_connection(args: &Args) -> Result<Connection> {
    if let Some(dsn) = &args.dsn {
        parse_dsn(dsn)
    } else if let Ok(env_dsn) = env::var("MAIL_URL") {
        parse_dsn(&env_dsn)
    } else {
        let host = args
            .host
            .clone()
            .ok_or_else(|| anyhow!("--host is required when --dsn is not provided"))?;
        let user = args
            .user
            .clone()
            .ok_or_else(|| anyhow!("--user is required when --dsn is not provided"))?;
        let pass = args
            .pass
            .clone()
            .ok_or_else(|| anyhow!("--pass is required when --dsn is not provided"))?;
        let port = args.port.unwrap_or(587);
        Ok(Connection {
            host,
            port,
            auth: Some(Auth { user, pass }),
            security: TransportSecurity::TlsWrapper,
        })
    }
}

fn parse_dsn(dsn: &str) -> Result<Connection> {
    let url = Url::parse(dsn).with_context(|| format!("invalid DSN: {dsn}"))?;
    let (security, default_port) = match url.scheme() {
        "smtp" => (TransportSecurity::Plain, 587),
        "smtps" => (TransportSecurity::TlsWrapper, 465),
        other => {
            return Err(anyhow!("unsupported DSN scheme: {other}"));
        }
    };
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("DSN must include host"))?
        .to_string();
    let port = url.port().unwrap_or(default_port);
    let user = url.username().to_string();
    let auth = if user.is_empty() {
        None
    } else {
        let pass = url
            .password()
            .ok_or_else(|| anyhow!("DSN must include password when username is provided"))?
            .to_string();
        Some(Auth { user, pass })
    };

    Ok(Connection {
        host,
        port,
        auth,
        security,
    })
}

fn build_message(args: &Args) -> Result<Message> {
    let mut builder = Message::builder().from(parse_mailbox(&args.from)?);

    for addr in &args.to {
        builder = builder.to(parse_mailbox(addr)?);
    }
    for addr in &args.cc {
        builder = builder.cc(parse_mailbox(addr)?);
    }
    for addr in &args.bcc {
        builder = builder.bcc(parse_mailbox(addr)?);
    }

    builder = apply_extra_headers(builder, &args.headers)?;
    builder = builder.subject(args.subject.clone());

    let base = compose_base_body(args)?;
    let email = if args.attachments.is_empty() {
        match base {
            BodyPart::Single(part) => builder.singlepart(part)?,
            BodyPart::Multi(multi) => builder.multipart(multi)?,
        }
    } else {
        let mut mixed = match base {
            BodyPart::Single(part) => MultiPart::mixed().singlepart(part),
            BodyPart::Multi(multi) => MultiPart::mixed().multipart(multi),
        };
        for attachment in &args.attachments {
            mixed = mixed.singlepart(load_attachment(attachment)?);
        }
        builder.multipart(mixed)?
    };

    Ok(email)
}

enum BodyPart {
    Single(SinglePart),
    Multi(MultiPart),
}

fn compose_base_body(args: &Args) -> Result<BodyPart> {
    match (&args.text, &args.html) {
        (Some(text), Some(html)) => {
            let alternative = MultiPart::alternative()
                .singlepart(SinglePart::plain(text.clone()))
                .singlepart(SinglePart::html(html.clone()));
            Ok(BodyPart::Multi(alternative))
        }
        (Some(text), None) => Ok(BodyPart::Single(SinglePart::plain(text.clone()))),
        (None, Some(html)) => Ok(BodyPart::Single(SinglePart::html(html.clone()))),
        (None, None) => Err(anyhow!("provide --text and/or --html for message body")),
    }
}

fn parse_mailbox(value: &str) -> Result<Mailbox> {
    value
        .parse()
        .with_context(|| format!("invalid email address: {value}"))
}

fn load_attachment(path: &Path) -> Result<SinglePart> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("attachment must have a valid filename: {}", path.display()))?;
    let data =
        fs::read(path).with_context(|| format!("failed to read attachment {}", path.display()))?;
    let mime = mime_guess::from_path(path).first_or(mime::APPLICATION_OCTET_STREAM);
    let content_type = ContentType::parse(mime.as_ref())
        .map_err(|_| anyhow!("invalid MIME type for attachment: {}", mime))?;

    Ok(Attachment::new(filename.to_string()).body(data, content_type))
}

fn apply_extra_headers(
    mut builder: lettre::message::MessageBuilder,
    headers: &[String],
) -> Result<lettre::message::MessageBuilder> {
    for raw in headers {
        let (name, value) = raw
            .split_once(':')
            .ok_or_else(|| anyhow!("invalid header format: expected Name:Value"))?;
        let trimmed_name = name.trim();
        let trimmed_value = value.trim();
        let header_name = HeaderName::new_from_ascii(trimmed_name.to_string())
            .map_err(|_| anyhow!("invalid header name: {trimmed_name}"))?;
        builder = builder.raw_header(HeaderValue::new(header_name, trimmed_value.to_string()));
    }
    Ok(builder)
}
