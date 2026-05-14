use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use kb_core::Source;
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(name = "kb", about = "KB · delta knowledge service · CLI")]
struct Cli {
    /// kb-server base URL (e.g. https://kb.example.com).
    #[arg(long, env = "KB_URL", global = true)]
    url: Option<String>,

    /// Bearer token. Falls back to KB_TOKEN env or ~/.config/kb/config.toml.
    #[arg(long, env = "KB_TOKEN", global = true)]
    token: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Look up an entry by ULID.
    Get { id: String },
    /// Most recently updated entries.
    Recent {
        #[arg(short, long, default_value = "20")]
        n: u32,
    },
    /// Search with optional FTS query and kind filter.
    Search {
        /// FTS query (simple_query mode). Empty = structured search only.
        #[arg(long)]
        q: Option<String>,
        /// Restrict by kind. Repeatable. (Fact|ProblemSolution|Lesson|Decision|Heuristic)
        #[arg(long = "kind")]
        kinds: Vec<String>,
        /// tag prefix to match (e.g. `network/`).
        #[arg(long = "tag")]
        tag: Vec<String>,
        #[arg(long, default_value = "20")]
        limit: u32,
        #[arg(long)]
        include_deprecated: bool,
    },
    /// Show stats (counts by kind, active vs deprecated).
    Stats,
    /// Write a Fact entry. (Other kinds get their own subcommands as needed.)
    WriteFact {
        #[arg(long)]
        title: String,
        #[arg(long)]
        claim: String,
        /// Evidence lines of form `KIND::content`. Repeatable.
        #[arg(long = "evidence")]
        evidence: Vec<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long, default_value = "")]
        body: String,
    },
    /// Deprecate an entry (no successor).
    Deprecate { id: String, reason: String },
    /// Physically delete an entry. Cascades over the supersede chain and
    /// rewrites [[ulid]] body refs in surviving entries to `[[ulid → deleted]]`.
    /// Defaults to dry-run; pass --yes to actually apply.
    Purge {
        id: String,
        /// Actually perform the deletion. Without this, the call is a preview.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, serde::Deserialize, Default)]
struct FileConfig {
    url: Option<String>,
    token: Option<String>,
}

fn load_config() -> FileConfig {
    let Some(home) = dirs::config_dir() else {
        return FileConfig::default();
    };
    let path = home.join("kb").join("config.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return FileConfig::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

fn resolved_url(cli: &Cli, file: &FileConfig) -> Result<String> {
    cli.url
        .clone()
        .or_else(|| file.url.clone())
        .ok_or_else(|| anyhow!("--url / KB_URL / config 'url' not set"))
}

fn resolved_token(cli: &Cli, file: &FileConfig) -> Option<String> {
    cli.token.clone().or_else(|| file.token.clone())
}

#[derive(Clone)]
struct Client {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl Client {
    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut r = self.http.request(
            method,
            format!("{}{}", self.base.trim_end_matches('/'), path),
        );
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        r
    }

    async fn json_value(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let mut req = self.req(method, path);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.context("request failed")?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if !status.is_success() {
            return Err(anyhow!(
                "{} {}: {}",
                status,
                path,
                String::from_utf8_lossy(&bytes)
            ));
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let file = load_config();
    let base = resolved_url(&cli, &file)?;
    let token = resolved_token(&cli, &file);
    let client = Client {
        base,
        token,
        http: reqwest::Client::new(),
    };

    match cli.cmd {
        Cmd::Get { id } => {
            let v = client
                .json_value(reqwest::Method::GET, &format!("/api/entries/{id}"), None)
                .await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::Recent { n } => {
            let v = client
                .json_value(reqwest::Method::GET, &format!("/api/recent?n={n}"), None)
                .await?;
            print_hits(&v);
        }
        Cmd::Search {
            q,
            kinds,
            tag,
            limit,
            include_deprecated,
        } => {
            let body = json!({
                "kinds": kinds,
                "tag_prefixes": tag,
                "query": q,
                "limit": limit,
                "include_deprecated": include_deprecated,
            });
            let v = client
                .json_value(reqwest::Method::POST, "/api/search", Some(body))
                .await?;
            print_hits(&v);
        }
        Cmd::Stats => {
            let v = client
                .json_value(reqwest::Method::GET, "/api/stats", None)
                .await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::WriteFact {
            title,
            claim,
            evidence,
            tags,
            body,
        } => {
            let evidence_json: Vec<Value> = evidence
                .into_iter()
                .map(|line| {
                    let (kind, content) = line
                        .split_once("::")
                        .map(|(a, b)| (a.to_string(), b.to_string()))
                        .unwrap_or(("SelfVerification".into(), line.clone()));
                    json!({
                        "kind": kind,
                        "content": content,
                        "captured_at": chrono::Utc::now().to_rfc3339(),
                    })
                })
                .collect();
            let input = json!({
                "kind": "Fact",
                "title": title,
                "body": body,
                "tags": tags,
                "source": Source::Human,
                "claim": claim,
                "evidence": evidence_json,
            });
            let v = client
                .json_value(reqwest::Method::POST, "/api/entries", Some(input))
                .await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::Deprecate { id, reason } => {
            client
                .json_value(
                    reqwest::Method::POST,
                    &format!("/api/entries/{id}/deprecate"),
                    Some(json!({ "reason": reason })),
                )
                .await?;
            println!("ok");
        }
        Cmd::Purge { id, yes } => {
            let path = if yes {
                format!("/api/entries/{id}")
            } else {
                format!("/api/entries/{id}?dry_run=true")
            };
            let v = client
                .json_value(reqwest::Method::DELETE, &path, None)
                .await?;
            print_purge(&v, yes);
        }
    }
    Ok(())
}

fn print_purge(v: &Value, applied: bool) {
    let dry = v
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(!applied);
    let purged = v.get("purged").and_then(Value::as_array);
    let rewritten = v.get("rewritten").and_then(Value::as_array);

    let header = if dry { "Would purge" } else { "Purged" };
    if let Some(arr) = purged {
        if arr.is_empty() {
            println!("(nothing to purge)");
            return;
        }
        println!("{header} {} entries:", arr.len());
        for e in arr {
            let id = e.get("id").and_then(Value::as_str).unwrap_or("?");
            let kind = e.get("kind").and_then(Value::as_str).unwrap_or("?");
            let title = e.get("title").and_then(Value::as_str).unwrap_or("");
            println!("  [{kind:16}] {id}  {title}");
        }
    }

    let action = if dry { "Would rewrite" } else { "Rewrote" };
    if let Some(arr) = rewritten {
        if !arr.is_empty() {
            println!("{action} body in {} other entries:", arr.len());
            for e in arr {
                let id = e.get("id").and_then(Value::as_str).unwrap_or("?");
                let n = e.get("refs_count").and_then(Value::as_u64).unwrap_or(0);
                let plural = if n == 1 { "" } else { "s" };
                println!("  {id}  ({n} reference{plural})");
            }
        }
    }

    if dry {
        println!();
        println!("Re-run with --yes to apply.");
    }
}

fn print_hits(v: &Value) {
    let Some(arr) = v.as_array() else {
        println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
        return;
    };
    if arr.is_empty() {
        println!("(no hits)");
        return;
    }
    for hit in arr {
        let kind = hit.get("kind").and_then(Value::as_str).unwrap_or("?");
        let id = hit.get("id").and_then(Value::as_str).unwrap_or("?");
        let title = hit.get("title").and_then(Value::as_str).unwrap_or("");
        let depr = hit.get("deprecated_at").is_some_and(|v| !v.is_null());
        let score = hit.get("score").and_then(Value::as_f64);
        let tag = if depr { " [deprecated]" } else { "" };
        match score {
            Some(s) => println!("[{kind:16}] {id}  {title}  ({s:.3}){tag}"),
            None => println!("[{kind:16}] {id}  {title}{tag}"),
        }
    }
}
