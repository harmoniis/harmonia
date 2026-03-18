use clap::Parser;
use harmonia_hfetch::{fetch, fetch_with_security_wrap, FetchOptions, Method};

#[derive(Parser)]
#[command(
    name = "hfetch",
    version = "0.1.8",
    about = "Secure HTTP fetch with signal-integrity protection"
)]
struct Cli {
    /// URL to fetch
    url: String,

    /// HTTP method
    #[arg(short = 'X', long = "method", default_value = "GET")]
    method: String,

    /// Request headers (repeatable: -H "Key: Value")
    #[arg(short = 'H', long = "header")]
    headers: Vec<String>,

    /// Request body
    #[arg(short = 'd', long = "data")]
    data: Option<String>,

    /// Bearer token for Authorization header
    #[arg(long = "bearer")]
    bearer: Option<String>,

    /// Timeout in milliseconds
    #[arg(long = "timeout", default_value = "10000")]
    timeout: u64,

    /// Max response size in bytes
    #[arg(long = "max-size", default_value = "2097152")]
    max_size: usize,

    /// Raw mode: skip security boundary wrapping (SSRF protection still applies)
    #[arg(long = "raw")]
    raw: bool,

    /// Show response headers
    #[arg(short = 'i', long = "include")]
    include_headers: bool,

    /// Quiet mode: only output body
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();

    let mut headers = Vec::new();
    for h in &cli.headers {
        if let Some((key, value)) = h.split_once(':') {
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
    }

    let opts = FetchOptions {
        method: Method::from_str(&cli.method),
        headers,
        body: cli.data.clone(),
        timeout_ms: cli.timeout,
        max_response_bytes: cli.max_size,
        auth_bearer: cli.bearer.clone(),
    };

    let result = if cli.raw {
        fetch(&cli.url, &opts)
    } else {
        fetch_with_security_wrap(&cli.url, &opts)
    };

    match result {
        Ok(resp) => {
            if !cli.quiet {
                eprintln!(
                    "HTTP {} | dissonance: {:.4}{}",
                    resp.status,
                    resp.dissonance,
                    if resp.injection_detected {
                        " | INJECTION DETECTED"
                    } else {
                        ""
                    }
                );
            }
            if cli.include_headers {
                for (k, v) in &resp.headers {
                    println!("{k}: {v}");
                }
                println!();
            }
            print!("{}", resp.body);
        }
        Err(e) => {
            eprintln!("hfetch error: {e}");
            std::process::exit(1);
        }
    }
}
