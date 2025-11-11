use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use clap::{ArgAction, Parser, ValueEnum};
use lettre::{
    SmtpTransport, Transport,
    message::{
        Attachment, Mailbox, Message, MultiPart, SinglePart,
        dkim::{DkimConfig, DkimSigningAlgorithm, DkimSigningKey},
        header::{ContentType, HeaderName, HeaderValue},
    },
    transport::smtp::authentication::Credentials,
};
use mime_guess::mime;
use regex::Regex;
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "wirepost",
    about = "Send an email via SMTP",
    version,
    after_help = "Environment variables: MAIL_URL supplies the DSN, MAIL_FROM supplies the sender address."
)]
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
    #[arg(long)]
    from: Option<String>,
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
    /// Plain-text body sourced from file
    #[arg(long = "text-file")]
    text_file: Option<PathBuf>,
    /// HTML body
    #[arg(long)]
    html: Option<String>,
    /// HTML body sourced from file
    #[arg(long = "html-file")]
    html_file: Option<PathBuf>,
    /// File attachments (repeatable)
    #[arg(long = "attach", action = ArgAction::Append)]
    attachments: Vec<PathBuf>,
    /// Print the fully formatted message instead of (or in addition to) sending
    #[arg(long)]
    print: bool,
    /// Additional headers in the form `Name: Value` (repeatable)
    #[arg(long = "header", action = ArgAction::Append)]
    headers: Vec<String>,
    /// Template variables used inside subject/body placeholders `{{key}}`
    #[arg(long = "var", action = ArgAction::Append)]
    vars: Vec<String>,
    /// Verbose logging for SMTP activity
    #[arg(long)]
    verbose: bool,
    /// Maximum SMTP send attempts
    #[arg(long = "max-attempts", default_value_t = 3)]
    max_attempts: u32,
    /// Initial backoff delay in milliseconds
    #[arg(long = "backoff-ms", default_value_t = 1_000)]
    backoff_ms: u64,
    /// Backoff multiplier applied after each failure
    #[arg(long = "backoff-factor", default_value_t = 2.0)]
    backoff_factor: f64,
    /// DKIM selector (requires domain and key)
    #[arg(long = "dkim-selector")]
    dkim_selector: Option<String>,
    /// DKIM domain (requires selector and key)
    #[arg(long = "dkim-domain")]
    dkim_domain: Option<String>,
    /// Path to DKIM private key (PKCS#1 for RSA or base64 for ed25519)
    #[arg(long = "dkim-key")]
    dkim_key: Option<PathBuf>,
    /// DKIM signing algorithm
    #[arg(long = "dkim-algorithm", value_enum, default_value = "rsa")]
    dkim_algorithm: DkimAlgorithm,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum DkimAlgorithm {
    Rsa,
    Ed25519,
}

impl DkimAlgorithm {
    fn to_lettre(self) -> DkimSigningAlgorithm {
        match self {
            DkimAlgorithm::Rsa => DkimSigningAlgorithm::Rsa,
            DkimAlgorithm::Ed25519 => DkimSigningAlgorithm::Ed25519,
        }
    }
}

struct Connection {
    host: String,
    port: u16,
    auth: Option<Auth>,
}

struct Auth {
    user: String,
    pass: String,
}

struct BodySource {
    text: Option<String>,
    html: Option<String>,
}

struct RenderedContent {
    subject: String,
    text: Option<String>,
    html: Option<String>,
    headers: Vec<String>,
}

type TemplateVars = HashMap<String, String>;

fn main() -> Result<()> {
    let args = Args::parse();
    if args.max_attempts == 0 {
        return Err(anyhow!("--max-attempts must be at least 1"));
    }

    let vars = parse_vars(&args.vars)?;
    let sources = load_body_sources(&args)?;
    let rendered = render_content(&args, &vars, &sources);
    let conn = resolve_connection(&args)?;
    let from = resolve_from(&args)?;
    log_verbose(
        args.verbose,
        &format!("SMTP target {}:{}", conn.host, conn.port),
    );

    let mut message = build_message(&args, &rendered, &from)?;
    if let Some(dkim_config) = load_dkim_config(&args)? {
        log_verbose(args.verbose, "Applying DKIM signature");
        message.sign(&dkim_config);
    }

    if args.print {
        let output = message.formatted();
        println!("{}", String::from_utf8_lossy(&output));
        log_verbose(
            args.verbose,
            "Skipping SMTP send because --print was provided",
        );
        return Ok(());
    }

    let mut builder = SmtpTransport::builder_dangerous(&conn.host).port(conn.port);
    if let Some(auth) = &conn.auth {
        builder = builder.credentials(Credentials::new(auth.user.clone(), auth.pass.clone()));
    }
    let transport = builder.build();

    send_with_retry(&transport, &message, &args)?;

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
        })
    }
}

fn parse_dsn(dsn: &str) -> Result<Connection> {
    let normalized = if dsn.contains("://") {
        dsn.to_string()
    } else {
        format!("smtp://{dsn}")
    };
    let url = Url::parse(&normalized).with_context(|| format!("invalid DSN: {dsn}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("DSN must include host"))?
        .to_string();
    let port = url.port().unwrap_or(587);
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

    Ok(Connection { host, port, auth })
}

fn build_message(args: &Args, rendered: &RenderedContent, from: &str) -> Result<Message> {
    let mut builder = Message::builder().from(parse_wirepostbox(from)?);

    for addr in &args.to {
        builder = builder.to(parse_wirepostbox(addr)?);
    }
    for addr in &args.cc {
        builder = builder.cc(parse_wirepostbox(addr)?);
    }
    for addr in &args.bcc {
        builder = builder.bcc(parse_wirepostbox(addr)?);
    }

    builder = apply_extra_headers(builder, &rendered.headers)?;
    builder = builder.subject(rendered.subject.clone());

    let base = compose_base_body(rendered)?;
    let ewirepost = if args.attachments.is_empty() {
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

    Ok(ewirepost)
}

enum BodyPart {
    Single(SinglePart),
    Multi(MultiPart),
}

fn compose_base_body(rendered: &RenderedContent) -> Result<BodyPart> {
    match (&rendered.text, &rendered.html) {
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

fn parse_wirepostbox(value: &str) -> Result<Mailbox> {
    value
        .parse()
        .with_context(|| format!("invalid ewirepost address: {value}"))
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

fn parse_vars(entries: &[String]) -> Result<TemplateVars> {
    let mut vars = HashMap::new();
    for entry in entries {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --var, expected key=value"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow!("template variable names cannot be empty"));
        }
        vars.insert(key.to_string(), value.to_string());
    }
    Ok(vars)
}

fn apply_template(input: &str, vars: &TemplateVars) -> String {
    if vars.is_empty() {
        return input.to_string();
    }

    let re = Regex::new(r"\{\{\s*([A-Za-z0-9_\-\.]+)\s*\}\}").expect("valid variable regex");

    re.replace_all(input, |caps: &regex::Captures| {
        let key = &caps[1];
        if let Some(value) = vars.get(key) {
            value.clone()
        } else {
            caps.get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        }
    })
    .into_owned()
}

fn render_content(args: &Args, vars: &TemplateVars, sources: &BodySource) -> RenderedContent {
    RenderedContent {
        subject: apply_template(&args.subject, vars),
        text: sources.text.as_ref().map(|text| apply_template(text, vars)),
        html: sources.html.as_ref().map(|html| apply_template(html, vars)),
        headers: args
            .headers
            .iter()
            .map(|header| apply_template(header, vars))
            .collect(),
    }
}

fn load_body_sources(args: &Args) -> Result<BodySource> {
    Ok(BodySource {
        text: resolve_body_source("text", &args.text, &args.text_file)?,
        html: resolve_body_source("html", &args.html, &args.html_file)?,
    })
}

fn resolve_body_source(
    label: &str,
    inline: &Option<String>,
    file: &Option<PathBuf>,
) -> Result<Option<String>> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(anyhow!(
            "provide either --{label} or --{label}-file, not both"
        )),
        (Some(value), None) => Ok(Some(value.clone())),
        (None, Some(path)) => {
            let data = fs::read_to_string(path)
                .with_context(|| format!("failed to read {label} body from {}", path.display()))?;
            Ok(Some(data))
        }
        (None, None) => Ok(None),
    }
}

fn resolve_from(args: &Args) -> Result<String> {
    if let Some(from) = &args.from {
        if !from.trim().is_empty() {
            return Ok(from.clone());
        }
    }
    if let Ok(env_from) = env::var("MAIL_FROM") {
        if !env_from.trim().is_empty() {
            return Ok(env_from);
        }
    }
    Err(anyhow!("provide --from or set MAIL_FROM"))
}

fn load_dkim_config(args: &Args) -> Result<Option<DkimConfig>> {
    match (&args.dkim_selector, &args.dkim_domain, &args.dkim_key) {
        (None, None, None) => Ok(None),
        (Some(selector), Some(domain), Some(path)) => {
            let key = fs::read_to_string(path)
                .with_context(|| format!("failed to read DKIM key {}", path.display()))?;
            let signing_key = DkimSigningKey::new(&key, args.dkim_algorithm.to_lettre())
                .context("failed to parse DKIM signing key")?;
            Ok(Some(DkimConfig::default_config(
                selector.clone(),
                domain.clone(),
                signing_key,
            )))
        }
        _ => Err(anyhow!(
            "--dkim-selector, --dkim-domain, and --dkim-key must be provided together"
        )),
    }
}

fn send_with_retry(wirepost: &SmtpTransport, message: &Message, args: &Args) -> Result<()> {
    let mut attempt = 1;
    let mut delay = Duration::from_millis(args.backoff_ms.max(1));
    loop {
        log_verbose(args.verbose, &format!("Sending attempt {attempt}"));
        match wirepost.send(message) {
            Ok(_) => {
                log_verbose(
                    args.verbose,
                    &format!("SMTP send succeeded on attempt {attempt}"),
                );
                return Ok(());
            }
            Err(err) => {
                let error = anyhow!(err);
                if attempt >= args.max_attempts {
                    return Err(error).context("failed to send message via SMTP");
                }
                log_verbose(
                    args.verbose,
                    &format!(
                        "Attempt {attempt} failed: {error}. Retrying in {}ms",
                        delay.as_millis()
                    ),
                );
                thread::sleep(delay);
                delay = next_delay(delay, args.backoff_factor);
                attempt += 1;
            }
        }
    }
}

fn next_delay(current: Duration, factor: f64) -> Duration {
    let clamped = if factor < 1.0 { 1.0 } else { factor };
    let millis = ((current.as_millis() as f64) * clamped).round() as u64;
    Duration::from_millis(millis.max(1))
}

fn log_verbose(enabled: bool, message: &str) {
    if enabled {
        eprintln!("[wirepost] {message}");
    }
}
