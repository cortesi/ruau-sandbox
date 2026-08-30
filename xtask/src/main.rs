//! Project automation tasks.

#![allow(clippy::missing_docs_in_private_items)]

use std::{
    fs,
    fs::File,
    io::{Cursor, ErrorKind},
    net::{Ipv4Addr, SocketAddrV4},
    path::{Component, Path, PathBuf},
    process::{self, Command},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tiny_http::{Header, Response, Server, StatusCode};
use xshell::{Shell, cmd};

const DIST_HOST: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
const DEFAULT_DIST_PORT: u16 = 8000;
const WASM_BINDGEN_VERSION: &str = "0.2.126";

#[derive(Debug, Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: CommandName,
}

#[derive(Debug, Subcommand)]
enum CommandName {
    /// Web build and serve tasks.
    #[command(subcommand)]
    Web(WebCommand),
}

#[derive(Debug, Subcommand)]
enum WebCommand {
    /// Install toolchain requirements for web builds.
    Setup,
    /// Launch the wasm development server.
    Serve,
    /// Build the production wasm bundle into dist/.
    Build,
    /// Serve dist/ on http://127.0.0.1:<port>.
    ServeDist {
        /// Port to bind.
        #[arg(default_value_t = DEFAULT_DIST_PORT)]
        port: u16,
    },
}

#[derive(Debug, Clone)]
struct RepoPaths {
    root: PathBuf,
    dist: PathBuf,
    dev_index_html: PathBuf,
    raw_wasm: PathBuf,
}

impl RepoPaths {
    fn discover() -> Result<Self> {
        let xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = xtask_dir
            .parent()
            .context("xtask crate must live at <repo>/xtask")?
            .to_path_buf();

        Ok(Self {
            dist: root.join("dist"),
            dev_index_html: root
                .join("crates")
                .join("ruau-sandbox-gui")
                .join("assets")
                .join("index.html"),
            raw_wasm: root
                .join("target")
                .join("wasm32-unknown-unknown")
                .join("wasm-release")
                .join("ruau-sandbox-web.wasm"),
            root,
        })
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let paths = RepoPaths::discover()?;

    match cli.command {
        CommandName::Web(command) => match command {
            WebCommand::Setup => web_setup(&paths),
            WebCommand::Serve => web_serve(&paths),
            WebCommand::Build => web_build(&paths),
            WebCommand::ServeDist { port } => web_serve_dist(&paths, port),
        },
    }
}

fn repo_shell(paths: &RepoPaths) -> Result<Shell> {
    let sh = Shell::new()?;
    sh.change_dir(&paths.root);
    Ok(sh)
}

fn web_setup(paths: &RepoPaths) -> Result<()> {
    let sh = repo_shell(paths)?;
    cmd!(sh, "rustup target add wasm32-unknown-unknown").run()?;
    cmd!(sh, "cargo install -f wasm-server-runner").run()?;
    install_wasm_bindgen_cli(paths)?;
    println!("Setup complete.");
    Ok(())
}

fn web_serve(paths: &RepoPaths) -> Result<()> {
    let sh = repo_shell(paths)?;
    if !paths.dev_index_html.is_file() {
        anyhow::bail!(
            "missing dev index html at {}",
            paths.dev_index_html.display()
        );
    }
    sh.set_var(
        "WASM_SERVER_RUNNER_CUSTOM_INDEX_HTML",
        &paths.dev_index_html,
    );
    cmd!(
        sh,
        "cargo run --target wasm32-unknown-unknown --bin ruau-sandbox-web"
    )
    .run()?;
    Ok(())
}

fn web_build(paths: &RepoPaths) -> Result<()> {
    let sh = repo_shell(paths)?;
    cmd!(
        sh,
        "cargo build --target wasm32-unknown-unknown --bin ruau-sandbox-web --profile wasm-release"
    )
    .run()?;

    ensure_raw_wasm_exists(paths)?;
    prepare_dist(paths)?;

    if has_command("wasm-bindgen")? {
        wasm_bindgen(paths)?;
        optimize_wasm(paths)?;
        write_dist_index(paths, production_index_html())?;
    } else {
        emit_fallback_bundle(paths)?;
    }

    println!("Build complete. Serve dist/ with `cargo xtask web serve-dist 8000`.");
    Ok(())
}

fn web_serve_dist(paths: &RepoPaths, port: u16) -> Result<()> {
    ensure_dist_ready(paths)?;

    let addr = SocketAddrV4::new(DIST_HOST, port);
    let server =
        Server::http(addr).map_err(|err| anyhow::anyhow!("failed to bind to {addr}: {err}"))?;
    println!("Serving dist/ on http://{DIST_HOST}:{port}");

    for request in server.incoming_requests() {
        handle_dist_request(paths, request)?;
    }
    Ok(())
}

fn ensure_raw_wasm_exists(paths: &RepoPaths) -> Result<()> {
    if paths.raw_wasm.is_file() {
        return Ok(());
    }

    anyhow::bail!("expected wasm artifact at {}", paths.raw_wasm.display());
}

fn prepare_dist(paths: &RepoPaths) -> Result<()> {
    match fs::remove_dir_all(&paths.dist) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err).context("failed to remove dist directory"),
    }
    fs::create_dir_all(&paths.dist).context("failed to create dist directory")?;
    Ok(())
}

fn wasm_bindgen(paths: &RepoPaths) -> Result<()> {
    ensure_wasm_bindgen_cli(paths)?;
    let sh = repo_shell(paths)?;
    let dist = &paths.dist;
    let raw_wasm = &paths.raw_wasm;
    cmd!(
        sh,
        "wasm-bindgen --target web --no-typescript --out-dir {dist} --out-name ruau-sandbox-web {raw_wasm}"
    )
    .run()?;
    Ok(())
}

fn optimize_wasm(paths: &RepoPaths) -> Result<()> {
    let bg_wasm = paths.dist.join("ruau-sandbox-web_bg.wasm");
    if bg_wasm.is_file() && has_command("wasm-opt")? {
        let sh = repo_shell(paths)?;
        cmd!(sh, "wasm-opt -Oz -o {bg_wasm} {bg_wasm}").run()?;
    }
    Ok(())
}

fn emit_fallback_bundle(paths: &RepoPaths) -> Result<()> {
    let raw_out = paths.dist.join("ruau-sandbox-web.wasm");
    fs::copy(&paths.raw_wasm, raw_out).context("failed to copy raw wasm artifact")?;
    write_dist_index(paths, fallback_index_html())?;
    Ok(())
}

fn ensure_dist_ready(paths: &RepoPaths) -> Result<()> {
    if paths.dist.join("index.html").is_file() {
        return Ok(());
    }
    anyhow::bail!("dist/index.html not found. Run `cargo xtask web build` first.");
}

fn handle_dist_request(paths: &RepoPaths, request: tiny_http::Request) -> Result<()> {
    let Some(rel_path) = sanitize_request_path(request.url()) else {
        request.respond(not_found_response())?;
        return Ok(());
    };
    let path = paths.dist.join(rel_path);
    if !path.is_file() {
        request.respond(not_found_response())?;
        return Ok(());
    }

    let file = File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut response = Response::from_file(file);
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let content_type = Header::from_bytes("Content-Type", mime.essence_str())
        .map_err(|()| anyhow::anyhow!("invalid content type"))?;
    response.add_header(content_type);
    request.respond(response)?;
    Ok(())
}

fn sanitize_request_path(url: &str) -> Option<PathBuf> {
    let requested = url.trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };

    let mut out = PathBuf::new();
    for component in Path::new(requested).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn not_found_response() -> Response<Cursor<Vec<u8>>> {
    Response::from_string("Not Found").with_status_code(StatusCode(404))
}

fn has_command(name: &str) -> Result<bool> {
    Ok(Command::new(name).arg("--help").output().is_ok())
}

fn ensure_wasm_bindgen_cli(paths: &RepoPaths) -> Result<()> {
    let expected = format!("wasm-bindgen {WASM_BINDGEN_VERSION}");
    let installed = Command::new("wasm-bindgen").arg("--version").output();
    if let Ok(output) = installed {
        let version = String::from_utf8_lossy(&output.stdout);
        if version.trim() == expected {
            return Ok(());
        }
    }

    install_wasm_bindgen_cli(paths)
}

fn install_wasm_bindgen_cli(paths: &RepoPaths) -> Result<()> {
    let sh = repo_shell(paths)?;
    let version = WASM_BINDGEN_VERSION;
    cmd!(sh, "cargo install -f wasm-bindgen-cli --version {version}").run()?;
    Ok(())
}

fn write_dist_index(paths: &RepoPaths, html: &str) -> Result<()> {
    fs::write(paths.dist.join("index.html"), html).context("failed to write dist/index.html")
}

fn production_index_html() -> &'static str {
    r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Ruau Sandbox</title>
    <style>
        html, body { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #101218; }
        canvas { display: block; width: 100vw; height: 100vh; }
    </style>
</head>
<body>
    <canvas id="ruau-sandbox"></canvas>
    <script type="module">
        import init from './ruau-sandbox-web.js';
        init('./ruau-sandbox-web_bg.wasm');
    </script>
</body>
</html>
"#
}

fn fallback_index_html() -> &'static str {
    r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Ruau Sandbox</title></head>
<body>
    <p>This fallback bundle needs wasm-bindgen output. Install wasm-bindgen-cli and rebuild.</p>
</body>
</html>
"#
}
