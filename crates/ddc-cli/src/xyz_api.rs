//! The xyz frontends: HTTP REST + MCP for ddc's query commands
//! (github.com/ejfkdev/xyz-rust — one definition, three interfaces).
//!
//! Every ddc command is registered with the CLI channel SKIPPED: the
//! existing hand-rolled CLI keeps handling `ddc getclass ...` exactly as
//! before. What the xyz layer adds:
//!
//! - `ddc serve --addr :8080` — HTTP REST + `GET /openapi.json` +
//!   streamable MCP on the same port
//! - `ddc mcp stdio` / `ddc mcp http` — MCP tool server
//!
//! Enabled with `--features xyz` (xyz-rust is not on crates.io yet; the
//! optional git dependency keeps `cargo publish` working).

use std::path::PathBuf;

use xyz_rust::{errs, CliHints, Ctx, HTTPHints, MCPHints, XyzArgs};

use crate::api;

type XyzResult<T> = std::result::Result<T, errs::Error>;

fn p(input: &str) -> PathBuf {
    PathBuf::from(input)
}

fn map_err(e: anyhow::Error) -> errs::Error {
    // File-not-found and friends surface as 404; the rest as internal.
    let msg = e.to_string();
    let kind = if msg.contains("os error 2") || msg.contains("not found") {
        errs::Kind::NotFound
    } else {
        errs::Kind::Internal
    };
    errs::new(kind, format!("{e:#}"))
}

// ---- ddc.info ----------------------------------------------------------------

#[derive(XyzArgs)]
struct InfoArgs {
    #[xyz(desc = "input: .dex / .apk / .jar / .zip / .xapk / directory", required)]
    input: String,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn info(_ctx: &Ctx, a: &InfoArgs) -> XyzResult<api::InfoReport> {
    api::info_report(&p(&a.input)).map_err(map_err)
}

// ---- ddc.listclasses ---------------------------------------------------------

#[derive(XyzArgs)]
struct ListclassesArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "case-insensitive substring filter")]
    pattern: String,
    #[xyz(desc = "segment-exact class prefix scope (a.b matches a.b and a.b.*)")]
    prefix: String,
    #[xyz(desc = "flat full listing (default: compressed prefix frontier with counts)")]
    full: bool,
    #[xyz(desc = "aggregate at exactly N segments below the prefix (0 = adaptive frontier)")]
    depth: i64,
    #[xyz(desc = "row order (name = lexicographic, count = biggest groups first)", enum = "name,count")]
    sort: String,
    #[xyz(desc = "maximum rows to return (0 = unlimited)")]
    limit: i64,
    #[xyz(desc = "number of rows to skip")]
    offset: i64,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

/// One text line per row: package rows render as `path/ (classes)`, class
/// leaves as the bare name (grep-friendly). The CLI adds a JSON envelope
/// with totals; the REST/MCP channels currently return text rows only.
fn listclasses(_ctx: &Ctx, a: &ListclassesArgs) -> XyzResult<Vec<String>> {
    if a.limit < 0 || a.offset < 0 || a.depth < 0 {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "limit/offset/depth must be non-negative".to_string(),
        ));
    }
    if !a.sort.is_empty() && a.sort != "name" && a.sort != "count" {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            format!("sort must be name|count, got {:?}", a.sort),
        ));
    }
    if a.full && (a.depth > 0 || !a.sort.is_empty()) {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "depth/sort only apply to the summary view (the default); drop full".to_string(),
        ));
    }
    let prefix = {
        let n = api::normalize_class_prefix(&a.prefix);
        if n.is_empty() { None } else { Some(n) }
    };
    let limit = a.limit as usize;
    let offset = a.offset as usize;
    // 摘要前缀目录是默认视图；full=true 恢复平铺全量列表。
    if !a.full {
        let page = api::class_names(
            &p(&a.input),
            empty_or_some(&a.pattern),
            prefix.as_deref(),
            &a.dex,
            0,
            0,
        )
        .map_err(map_err)?;
        let depth = if a.depth == 0 { None } else { Some(a.depth as usize) };
        let mut rows = api::class_tree(&page.names, prefix.as_deref(), depth);
        if a.sort == "count" {
            rows.sort_by(|x, y| y.count.cmp(&x.count).then_with(|| x.path.cmp(&y.path)));
        }
        let rp = api::window_rows(rows, offset, limit);
        let mut out: Vec<String> = rp
            .rows
            .iter()
            .map(|r| match r.kind {
                api::TreeKind::Group => format!("{}/ ({})", r.path, r.count),
                api::TreeKind::Leaf if r.count > 1 => format!("{} ({})", r.path, r.count),
                api::TreeKind::Leaf => r.path.clone(),
            })
            .collect();
        if let Some(next) = rp.next_offset {
            out.push(format!(
                "(truncated: showing {} of {} rows; pass offset={} to continue)",
                rp.rows.len(),
                rp.total,
                next
            ));
        }
        return Ok(out);
    }
    let page = api::class_names(
        &p(&a.input),
        empty_or_some(&a.pattern),
        prefix.as_deref(),
        &a.dex,
        offset,
        limit,
    )
    .map_err(map_err)?;
    let mut out = page.names;
    let end = offset.saturating_add(out.len());
    if end < page.total {
        out.push(format!(
            "(truncated: showing {} of {} classes; pass offset={} to continue)",
            out.len(),
            page.total,
            end
        ));
    }
    Ok(out)
}

fn empty_or_some(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

// ---- ddc.getclass ------------------------------------------------------------

#[derive(XyzArgs)]
struct GetClassArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "class name, dotted or slashed", required)]
    class: String,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn getclass(_ctx: &Ctx, a: &GetClassArgs) -> XyzResult<api::ClassSource> {
    api::class_source(&[p(&a.input)], &a.class, &a.dex).map_err(map_err)
}

// ---- ddc.getmethod -----------------------------------------------------------

#[derive(XyzArgs)]
struct GetMethodArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "class.method (dotted), or a bare class for the whole class", required)]
    method: String,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn getmethod(_ctx: &Ctx, a: &GetMethodArgs) -> XyzResult<api::MethodSource> {
    api::method_source(&p(&a.input), &a.method, &a.dex).map_err(map_err)
}

// ---- ddc.findrefs ------------------------------------------------------------

#[derive(XyzArgs)]
struct FindRefsArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "reference kind (class = type alias)", required, enum = "string,type,class,method,field")]
    kind: String,
    #[xyz(desc = "query: literal substring / type name / member name", required)]
    query: String,
    #[xyz(desc = "owner class for method/field (exact unless --fuzzy-class)")]
    class: String,
    #[xyz(desc = "match the owner class fuzzily")]
    fuzzy_class: bool,
    #[xyz(desc = "flat hit rows (default: prefix frontier of owner classes)")]
    full: bool,
    #[xyz(desc = "segment-exact owner-class scope (a.b matches a.b and a.b.*)")]
    prefix: String,
    #[xyz(desc = "aggregate at exactly N segments below the prefix (0 = adaptive)")]
    depth: i64,
    #[xyz(desc = "row order (name = lexicographic, count = biggest groups first)", enum = "name,count")]
    sort: String,
    #[xyz(desc = "maximum rows to return (0 = unlimited)")]
    limit: i64,
    #[xyz(desc = "number of rows to skip")]
    offset: i64,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn findrefs(_ctx: &Ctx, a: &FindRefsArgs) -> XyzResult<Vec<serde_json::Value>> {
    if a.limit < 0 || a.offset < 0 || a.depth < 0 {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "limit/offset/depth must be non-negative".to_string(),
        ));
    }
    if !a.sort.is_empty() && a.sort != "name" && a.sort != "count" {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            format!("sort must be name|count, got {:?}", a.sort),
        ));
    }
    if a.full && (a.depth > 0 || !a.sort.is_empty()) {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "depth/sort only apply to the summary view (the default); drop full".to_string(),
        ));
    }
    let prefix = {
        let n = api::normalize_class_prefix(&a.prefix);
        if n.is_empty() { None } else { Some(n) }
    };
    let mut rows = api::ref_rows(
        &p(&a.input),
        &a.kind,
        &a.query,
        empty_or_some(&a.class),
        a.fuzzy_class,
        &a.dex,
    )
    .map_err(map_err)?;
    if let Some(pfx) = &prefix {
        rows.retain(|r| api::under_prefix(&r.class, pfx, '.'));
    }
    let limit = a.limit as usize;
    let offset = a.offset as usize;
    if !a.full {
        let keys: Vec<String> = rows.iter().map(|r| r.class.clone()).collect();
        let depth = if a.depth == 0 { None } else { Some(a.depth as usize) };
        let mut tree = api::prefix_tree(
            keys.iter().map(|k| (k.as_str(), 1usize)),
            prefix.as_deref(),
            depth,
            '.',
        );
        if a.sort == "count" {
            tree.sort_by(|x, y| y.count.cmp(&x.count).then_with(|| x.path.cmp(&y.path)));
        }
        let rp = api::window_rows(tree, offset, limit);
        let mut out: Vec<serde_json::Value> = rp
            .rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "path": r.path,
                    "kind": match r.kind {
                        api::TreeKind::Leaf => "class",
                        api::TreeKind::Group => "package",
                    },
                    "count": r.count,
                })
            })
            .collect();
        if let Some(next) = rp.next_offset {
            // 与树行同键：MCP 文本按首行键渲染表格，异键对象会被吞掉。
            out.push(serde_json::json!({
                "path": format!(
                    "(truncated: showing {} of {} rows; pass offset={} to continue)",
                    rp.rows.len(), rp.total, next
                ),
                "kind": "truncated",
                "count": 0,
            }));
        }
        return Ok(out);
    }
    let rp = api::window_rows(rows, offset, limit);
    let mut out: Vec<serde_json::Value> = rp
        .rows
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
        .collect();
    if let Some(next) = rp.next_offset {
        out.push(serde_json::json!({
            "dex": "",
            "kind": "truncated",
            "class": format!(
                "showing {} of {} hits; pass offset={} to continue",
                rp.rows.len(), rp.total, next
            ),
            "method": "",
            "refs": [],
        }));
    }
    Ok(out)
}

// ---- ddc.strings ---------------------------------------------------------------

#[derive(XyzArgs)]
struct StringsArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "substring filter (case-insensitive)")]
    filter: String,
    #[xyz(desc = "map each string to its referencing methods")]
    with_locations: bool,
    #[xyz(desc = "maximum rows to return (0 = unlimited)")]
    limit: i64,
    #[xyz(desc = "number of rows to skip")]
    offset: i64,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn strings(_ctx: &Ctx, a: &StringsArgs) -> XyzResult<Vec<serde_json::Value>> {
    if a.limit < 0 || a.offset < 0 {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "limit/offset must be non-negative".to_string(),
        ));
    }
    let offset = a.offset as usize;
    let limit = a.limit as usize;
    let mut out: Vec<serde_json::Value> = Vec::new();
    let total = api::string_scan_window(
        &p(&a.input),
        empty_or_some(&a.filter),
        a.with_locations,
        &a.dex,
        offset,
        limit,
        |r| out.push(serde_json::to_value(r).unwrap_or(serde_json::Value::Null)),
    )
    .map_err(map_err)?;
    let end = offset.saturating_add(out.len());
    if end < total {
        // 与 StringRow 同键（首行无 used_by 时不带该键），保证 MCP 表格可见。
        out.push(serde_json::json!({
            "dex": "",
            "string": format!(
                "(truncated: showing {} of {} strings; pass offset={} to continue)",
                out.len(), total, end
            ),
        }));
    }
    Ok(out)
}

// ---- ddc.hierarchy -------------------------------------------------------------

#[derive(XyzArgs)]
struct HierarchyArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "class name, dotted or slashed", required)]
    class: String,
    #[xyz(desc = "flat relation rows (default: per-relation prefix frontier)")]
    full: bool,
    #[xyz(desc = "segment-exact scope on the result classes (a.b matches a.b and a.b.*)")]
    prefix: String,
    #[xyz(desc = "aggregate at exactly N segments below the prefix (0 = adaptive)")]
    depth: i64,
    #[xyz(desc = "row order (name = lexicographic, count = biggest groups first)", enum = "name,count")]
    sort: String,
    #[xyz(desc = "maximum rows to return (0 = unlimited)")]
    limit: i64,
    #[xyz(desc = "number of rows to skip")]
    offset: i64,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn hierarchy(_ctx: &Ctx, a: &HierarchyArgs) -> XyzResult<Vec<serde_json::Value>> {
    if a.limit < 0 || a.offset < 0 || a.depth < 0 {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "limit/offset/depth must be non-negative".to_string(),
        ));
    }
    if !a.sort.is_empty() && a.sort != "name" && a.sort != "count" {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            format!("sort must be name|count, got {:?}", a.sort),
        ));
    }
    if a.full && (a.depth > 0 || !a.sort.is_empty()) {
        return Err(errs::new(
            errs::Kind::InvalidInput,
            "depth/sort only apply to the summary view (the default); drop full".to_string(),
        ));
    }
    let prefix = {
        let n = api::normalize_class_prefix(&a.prefix);
        if n.is_empty() { None } else { Some(n) }
    };
    let mut rows = api::hierarchy_rows(&p(&a.input), &a.class, &a.dex).map_err(map_err)?;
    if let Some(pfx) = &prefix {
        rows.retain(|r| {
            api::under_prefix(&hierarchy_plain_pub(&r.class).replace('/', "."), pfx, '.')
        });
    }
    let limit = a.limit as usize;
    let offset = a.offset as usize;
    if !a.full {
        const ORDER: [&str; 5] = ["class", "extends", "implements", "sub", "impl"];
        let depth = if a.depth == 0 { None } else { Some(a.depth as usize) };
        let mut combined: Vec<(usize, String, api::TreeRow)> = Vec::new();
        for (ri, rel) in ORDER.iter().enumerate() {
            let keys: Vec<String> = rows
                .iter()
                .filter(|r| r.relation == *rel)
                .map(|r| hierarchy_plain_pub(&r.class).replace('/', "."))
                .collect();
            if keys.is_empty() {
                continue;
            }
            let tree = api::prefix_tree(
                keys.iter().map(|k| (k.as_str(), 1usize)),
                prefix.as_deref(),
                depth,
                '.',
            );
            for row in tree {
                combined.push((ri, rel.to_string(), row));
            }
        }
        if a.sort == "count" {
            combined.sort_by(|x, y| {
                y.2.count
                    .cmp(&x.2.count)
                    .then_with(|| x.2.path.cmp(&y.2.path))
                    .then_with(|| x.0.cmp(&y.0))
            });
        } else {
            combined.sort_by(|x, y| x.0.cmp(&y.0).then_with(|| x.2.path.cmp(&y.2.path)));
        }
        let rp = api::window_rows(combined, offset, limit);
        let mut out: Vec<serde_json::Value> = rp
            .rows
            .iter()
            .map(|(_, rel, r)| {
                serde_json::json!({
                    "relation": rel,
                    "path": r.path,
                    "kind": match r.kind {
                        api::TreeKind::Leaf => "class",
                        api::TreeKind::Group => "package",
                    },
                    "count": r.count,
                })
            })
            .collect();
        if let Some(next) = rp.next_offset {
            out.push(serde_json::json!({
                "relation": "truncated",
                "path": format!(
                    "(showing {} of {} rows; pass offset={} to continue)",
                    rp.rows.len(), rp.total, next
                ),
                "kind": "truncated",
                "count": 0,
            }));
        }
        return Ok(out);
    }
    let rp = api::window_rows(rows, offset, limit);
    let mut out: Vec<serde_json::Value> = rp
        .rows
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
        .collect();
    if let Some(next) = rp.next_offset {
        out.push(serde_json::json!({
            "dex": "",
            "relation": "truncated",
            "class": format!(
                "showing {} of {} rows; pass offset={} to continue",
                rp.rows.len(), rp.total, next
            ),
        }));
    }
    Ok(out)
}

/// `L…;` descriptor rows (extends/implements) → plain name.
fn hierarchy_plain_pub(class: &str) -> &str {
    class
        .strip_prefix('L')
        .and_then(|s| s.strip_suffix(';'))
        .unwrap_or(class)
}

// ---- ddc.disasm ----------------------------------------------------------------

#[derive(XyzArgs)]
struct DisasmArgs {
    #[xyz(desc = "input: APK / dex / container", required)]
    input: String,
    #[xyz(desc = "class name, optionally Class.method", required)]
    target: String,
    #[xyz(desc = "restrict images whose entry name contains this (repeatable)")]
    dex: Vec<String>,
}

fn disasm(_ctx: &Ctx, a: &DisasmArgs) -> XyzResult<String> {
    api::disasm_text(&p(&a.input), &a.target, &a.dex).map_err(map_err)
}

// ---- ddc.manifest --------------------------------------------------------------

#[derive(XyzArgs)]
struct ManifestArgs {
    #[xyz(desc = "input: APK / container", required)]
    input: String,
    #[xyz(desc = "extract one component group", enum = "launcher,activity,service,receiver,provider,activity-alias,application,permission")]
    component: String,
}

fn manifest(_ctx: &Ctx, a: &ManifestArgs) -> XyzResult<String> {
    api::manifest_text(&p(&a.input), empty_or_some(&a.component)).map_err(map_err)
}

// ---- ddc.mainactivity ----------------------------------------------------------

#[derive(XyzArgs)]
struct MainActivityArgs {
    #[xyz(desc = "input: APK / container", required)]
    input: String,
}

fn mainactivity(_ctx: &Ctx, a: &MainActivityArgs) -> XyzResult<api::MainActivityReport> {
    api::main_activity(&p(&a.input)).map_err(map_err)
}

// ---- registration ---------------------------------------------------------------

/// The CLI channel is skipped on every command: the existing ddc CLI
/// surface is untouched, and `serve`/`mcp` (xyz mode words) are the only
/// new top-level words. Flip a command's CliHints to enable the xyz CLI
/// rendering for it later.
fn skip_cli() -> CliHints {
    CliHints { skip: true, ..Default::default() }
}

/// 空 method = xyz §11.1 默认双路由：GET 绑 query、POST 绑 JSON body
/// （query 同样生效，body 缺席键由 query 补齐）。
fn http(path: &str) -> HTTPHints {
    HTTPHints {
        method: String::new(),
        path: path.into(),
        ..Default::default()
    }
}

fn mcp_read() -> MCPHints {
    MCPHints {
        annotations: vec!["read".into()],
        ..Default::default()
    }
}

/// MCP 通道对列表类工具默认封顶 500 行：agent 拿到的单次结果有界，
/// 超出部分由响应末尾的 truncated 提示行给出 next_offset。CLI/REST
/// 不受影响（默认 0 = 不限）；显式传 limit=0 仍可拿全量。
fn mcp_capped() -> MCPHints {
    let mut h = mcp_read();
    h.fields.insert(
        "limit".to_string(),
        xyz_rust::MCPFieldHint {
            default: Some(serde_json::json!(500)),
            description: Some(
                "maximum rows (MCP default 500; 0 = unlimited)".to_string(),
            ),
        },
    );
    h
}

/// Register every ddc query command and dispatch `args` through the xyz
/// pipeline. Only reached with `serve` / `mcp` as the first argument
/// (see main); returns the process exit code.
pub fn run(args: Vec<String>) -> i32 {
    // MCP serverInfo.version / X-App-Version / _meta.xyz.app_version 用
    // ddc 自身版本，而不是 xyz 的默认 "dev"。必须在派发前调用。
    xyz_rust::set_version(env!("CARGO_PKG_VERSION"));
    xyz_rust::define("ddc.info", info)
        .summary("App context and per-image statistics")
        .cli(skip_cli())
        .http(http("/info"))
        .mcp(mcp_read())
        .also(&[
            &xyz_rust::define("ddc.listclasses", listclasses)
                .summary("Class structure as a prefix frontier with counts (full=true: flat names)")
                .cli(skip_cli())
                .http(http("/classes"))
                .mcp(mcp_capped()),
            &xyz_rust::define("ddc.getclass", getclass)
                .summary("Decompile one class (nested classes included)")
                .cli(skip_cli())
                .http(http("/class"))
                .mcp(mcp_read()),
            &xyz_rust::define("ddc.getmethod", getmethod)
                .summary("Decompile one method, all overloads")
                .cli(skip_cli())
                .http(http("/method"))
                .mcp(mcp_read()),
            &xyz_rust::define("ddc.findrefs", findrefs)
                .summary("Cross-references; default owner-class frontier (full=true: hit rows)")
                .cli(skip_cli())
                .http(http("/findrefs"))
                .mcp(mcp_capped()),
            &xyz_rust::define("ddc.strings", strings)
                .summary("String table; pageable with limit/offset (MCP caps at 500)")
                .cli(skip_cli())
                .http(http("/strings"))
                .mcp(mcp_capped()),
            &xyz_rust::define("ddc.hierarchy", hierarchy)
                .summary("Super/subtypes; default per-relation frontier (full=true: rows)")
                .cli(skip_cli())
                .http(http("/hierarchy"))
                .mcp(mcp_capped()),
            &xyz_rust::define("ddc.disasm", disasm)
                .summary("Raw bytecode listing of a class or method")
                .cli(skip_cli())
                .http(http("/disasm"))
                .mcp(mcp_read()),
            &xyz_rust::define("ddc.manifest", manifest)
                .summary("AndroidManifest.xml as text XML")
                .cli(skip_cli())
                .http(http("/manifest"))
                .mcp(mcp_read()),
            &xyz_rust::define("ddc.mainactivity", mainactivity)
                .summary("Package and launcher activity, verified in the dex")
                .cli(skip_cli())
                .http(http("/mainactivity"))
                .mcp(mcp_read()),
        ])
        .run_args(args)
}
