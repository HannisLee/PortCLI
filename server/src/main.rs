use std::sync::Arc;

use axum::{
    body::Body,
    http::{Response, Uri},
    routing::{get, post},
    Router,
};
use clap::{CommandFactory, Parser, Subcommand};
use tower_http::cors::CorsLayer;
use tracing::info;

#[path = "../core.rs"]
mod core;

use core::{AppState, ConfigStore, PartialUpdate, ProxyManager};

const INDEX_HTML: &str = include_str!("../web.html");

#[derive(Parser)]
#[command(name = "porthannis")]
#[command(about = "PortHannis - 轻量级端口转发管理器")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 列出所有转发条目
    List,

    /// 添加新的转发条目
    Add {
        /// 条目名称
        #[arg(short, long)]
        name: String,

        /// 源端口（监听端口）
        #[arg(short = 's', long)]
        source_port: u16,

        /// 目标地址
        #[arg(short = 'a', long)]
        target_address: String,

        /// 目标端口
        #[arg(short = 't', long)]
        target_port: u16,
    },

    /// 修改已有的转发条目
    Modify {
        /// 条目 ID
        id: String,

        /// 新名称
        #[arg(short, long)]
        name: Option<String>,

        /// 新源端口
        #[arg(short = 's', long)]
        source_port: Option<u16>,

        /// 新目标地址
        #[arg(short = 'a', long)]
        target_address: Option<String>,

        /// 新目标端口
        #[arg(short = 't', long)]
        target_port: Option<u16>,

        /// 启用/禁用 (true/false)
        #[arg(short, long)]
        enabled: Option<bool>,
    },

    /// 启动 Web 管理界面
    Serve {
        /// 监听地址
        #[arg(long, default_value = "127.0.0.1:7777")]
        addr: String,

        /// 不自动打开浏览器
        #[arg(long)]
        no_open: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            Cli::command().print_help()?;
            println!();
        }
        Some(Commands::List) => cmd_list()?,
        Some(Commands::Add {
            name,
            source_port,
            target_address,
            target_port,
        }) => cmd_add(name, source_port, target_address, target_port)?,
        Some(Commands::Modify {
            id,
            name,
            source_port,
            target_address,
            target_port,
            enabled,
        }) => cmd_modify(id, name, source_port, target_address, target_port, enabled)?,
        Some(Commands::Serve { addr, no_open }) => {
            run_server(addr, no_open)?;
        }
    }

    Ok(())
}

fn cmd_list() -> anyhow::Result<()> {
    let store = ConfigStore::load()?;
    let entries = store.entries();

    if entries.is_empty() {
        println!("暂无转发条目。");
        return Ok(());
    }

    use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["ID", "名称", "源端口", "目标地址", "目标端口", "启用"]);

    for e in entries {
        table.add_row(vec![
            e.id.clone(),
            e.name.clone(),
            e.source_port.to_string(),
            e.target_address.clone(),
            e.target_port.to_string(),
            if e.enabled { "是".to_string() } else { "否".to_string() },
        ]);
    }

    println!("{table}");
    Ok(())
}

fn cmd_add(
    name: String,
    source_port: u16,
    target_address: String,
    target_port: u16,
) -> anyhow::Result<()> {
    let mut store = ConfigStore::load()?;
    let req = core::EntryRequest {
        name,
        source_address: "0.0.0.0".to_string(),
        source_port,
        target_address,
        target_port,
        enabled: true,
    };
    let entry = store.add_entry(req)?;
    println!("已添加条目: {} (ID: {})", entry.name, entry.id);
    Ok(())
}

fn cmd_modify(
    id: String,
    name: Option<String>,
    source_port: Option<u16>,
    target_address: Option<String>,
    target_port: Option<u16>,
    enabled: Option<bool>,
) -> anyhow::Result<()> {
    if name.is_none()
        && source_port.is_none()
        && target_address.is_none()
        && target_port.is_none()
        && enabled.is_none()
    {
        println!("未指定任何修改项。请使用 --name/--source-port/--target-address/--target-port/--enabled 参数。");
        return Ok(());
    }

    let mut store = ConfigStore::load()?;
    let updates = PartialUpdate {
        name,
        source_port,
        target_address,
        target_port,
        enabled,
    };
    let entry = store.update_entry_partial(&id, updates)?;
    println!("已更新条目: {} (ID: {})", entry.name, entry.id);
    Ok(())
}

#[tokio::main]
async fn run_server(addr: String, no_open: bool) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "porthannis=info,tower_http=info".into()),
        )
        .init();

    let manager = ProxyManager::new().await?;
    manager.auto_start_enabled().await;

    let app_state = AppState {
        manager: Arc::new(manager),
    };

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route(
            "/api/entries",
            get(core::list_entries).post(core::create_entry),
        )
        .route(
            "/api/entries/{id}",
            get(core::get_entry)
                .put(core::update_entry)
                .delete(core::delete_entry),
        )
        .route("/api/entries/{id}/start", post(core::start_entry))
        .route("/api/entries/{id}/stop", post(core::stop_entry))
        .route("/api/entries/{id}/status", get(core::get_entry_status))
        .route("/api/entries/{id}/logs", get(core::get_entry_logs))
        .fallback(serve_frontend)
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    println!("PortHannis 管理服务启动: http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("PortHannis 启动: http://{}", addr);

    if !no_open {
        let browser_url = format!("http://{}", addr);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if let Err(e) = opener::open(&browser_url) {
                tracing::debug!("自动打开浏览器失败: {}", e);
            }
        });
    }

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn serve_frontend(_uri: Uri) -> Response<Body> {
    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(INDEX_HTML))
        .unwrap()
}
