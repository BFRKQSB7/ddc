# xyz-rust 待修问题清单（来自 ddc 的 HTTP/MCP 深度测试）

- 测试环境：xyz-rust 0.4.11、rmcp 3.5.1、ddc 0.1.31（本地构建）
- 测试宿主：ddc（37 个 dex、21.3 万类的真实 APK）
- 传输：MCP stdio、MCP streamable HTTP（legacy session + `--stateless`）、REST `serve`
- 优先级：P1 = xyz 自身代码可修；P2 = rmcp 上游（xyz 需确认/上报或做薄包装）；P3 = 已修复项的防回归清单

---

## 0.4.13 验证结果（2026-10-21，ddc fc5e901）

| 条目 | 状态 |
|---|---|
| P1-1 未知 query 参数 | **已修复**：400 invalid_input + allowed 清单；`--xyz.allow-unknown-query` 可放开。ddc 10 个路由全部验证，schema 字段均在允许集内 |
| P1-2 `--json-response` | **已缓解**：legacy 模式启动告警（"has no effect without --stateless"）；`--stateless --json-response` 返回 `application/json` |
| P1-3 列表 envelope | 未做（可选特性，建议随 ddc issue #7 契约推进） |
| P2-4 id 类型 | **已改善**（rmcp 3.6.0）：null/浮点/对象 id 现在回 `-32600`，不再静默丢弃；仍不完全符合 JSON-RPC（响应不带 id 字段；null/数字本应合法） |
| P2-5 `tools/call` 缺 params | 仍为 `-32601` |
| P2-6 畸形 body 状态码 | 仍为 `415`（应 400/422） |
| P2-7 非 JSON stdio 行 | 仍静默忽略（无 -32700） |
| P2-8 `server/discover` 缺参 | **已修复**（rmcp 3.6.0）：统一 `-32602`（缺 _meta） |
| P2-9 DELETE 幂等 | 仍为 `202/202` |
| P3 防回归项 | 全部通过：版本门控、stateless 2026-07-28、10 工具、GET/POST 一致性、MCPFieldHint |

---

## P1. xyz 自身可修

### 1. REST 未知 query 参数被静默忽略（建议优先修）

**复现**（任意 xyz 应用，以 ddc 为例）：

```bash
ddc serve --addr 127.0.0.1:8080 &
curl 'http://127.0.0.1:8080/classes?input=app.apk&bogus_param=x'
# -> HTTP 200，返回完整结果（看起来像参数生效了）
```

**期望**：`400` + 标准失败信封（`kind: invalid_input`），消息列出该路由允许的参数名。客户端传入拼错/已移除的参数时应立即失败，而不是收到一份"看起来正常"的结果。

**根因**：`src/httpapi/mod.rs:308` 起的绑定循环只做"按字段名去 query 里取值"，从不反向检查 query 里是否存在未知键：

```rust
for f in &entry.root.children { /* match location: query/header/form/path */ }
```

**建议修法**：

1. 用 `parse_form_query(&query)`（同文件 `:407`）拿到 query 的全部键。
2. 允许集 = `entry.root.children` 中 `http.location` 为空或 `"query"` 的字段的 `http_name(f)`（`http_name` 在 `:399`）。`skip`/`header`/`path`/`form` 字段不从 query 绑定，不应计入（除非该路由确实接受同名 query——按实际 location 语义决定）。
3. 有任何未知键 → 返回现有的失败信封（复用 `write_coded_error` / `errs::Kind::InvalidInput`），消息形如 `unknown query parameter "bogus_param" (allowed: input, pattern, ...)`。
4. 建议加一个 Config 逃生开关（如 `Config.allow_unknown_query: bool`，默认 false = 严格），并在 spec 的 §11.1 补一句语义；OpenAPI 文档不需要变。
5. 测试：`src/httpapi/httpapi_test.rs` 增加"未知参数 400 / 已知参数 200 / 重复数组参数正常"三例。

**注意**：
- 语言协商走 `Accept-Language` 头（`:255`），不是 query 参数，不会被误伤。
- 不要把 JSON body 的未知键一起拒绝（除非同时更新 body 绑定逻辑与文档；本次诉求只针对 query）。
- 这是 ddc issue #7 里明确提出的契约。

### 2. `--json-response` 在默认（legacy session）模式下无效

**复现**：

```bash
ddc mcp http --addr 127.0.0.1:8080 --json-response &
curl -i -X POST 'http://127.0.0.1:8080/mcp' \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}'
# -> Content-Type: text/event-stream（仍走 SSE）
curl ... -H 'Accept: application/json'   # -> 406 "must accept both"
```

**期望**：设置 `--json-response` 后，单条终止响应以 `application/json` 返回；或者至少程序启动时明确提示"该选项仅在 `--stateless` 下生效"。

**根因**：`src/mcp/transport.rs:209` 正确地把选项写入了 `StreamableHttpServerConfig.json_response`，但 rmcp 3.5.1 只在 **stateless 路径**实现该行为（`rmcp-3.5.1/src/transport/streamable_http_server/tower.rs:1588` 与 `:2312`）；legacy session 的 `handle_post` 始终返回 SSE。另外 rmcp 的 Accept 校验（`tower.rs:2013-2023`）无条件要求同时包含两种 MIME，因此 JSON-only 客户端始终 406（这符合 MCP 规范，但让 `--json-response` 对 JSON-only 客户端没有意义）。

**建议修法**（按成本排序）：
1. 文档 + 启动警告：`--json-response` 未配 `--stateless` 时输出一行警告（或直接报错要求组合使用）。
2. 若要 legacy 模式也支持 JSON：在 axum 层把"单条消息的 SSE 响应"重写为 `application/json`（需处理 `data:` 帧解析与多消息回退），或向 rmcp 提交上游实现。
3. 保持 `--stateless --json-response` 现状（实测可用，返回 `application/json`）。

### 3.（可选特性）列表结果的 envelope / 分页元数据

**背景**：MCP 的 `structuredContent` 和 outputSchema 只支持 object 结果；REST/MCP 的数组结果无法携带 `total`/`offset`/`next_offset`。ddc 目前的绕行方案是在数组末尾追加一条与行同键的"truncated"提示（MCP 文本表格可见），但结构化客户端拿不到元数据。

**建议**：为命令增加可选的"分页结果"声明（例如输出类型为 `{rows, total, offset, limit, returned, next_offset}`）：
- REST：原样返回该对象；
- MCP：作为 `structuredContent`，并按 object 生成 outputSchema；
- 文本渲染：仍按 `rows` 出表格。
- 必须 opt-in（逐命令），不影响现有数组形状。

**注意**：这属于能力扩展，建议与 ddc issue #7 的契约一起定，不要作为默认行为。

---

## P2. rmcp 上游问题（xyz 需确认；能包装则包装，否则上报）

以下均可在 ddc 0.1.31 + xyz 0.4.11 复现，根因在 rmcp 的传输/分发层（xyz 使用的是 `rmcp::transport::stdio()` 与 `StreamableHttpService`，未自写解析）。

### 4. `id` 为 `null` / 浮点 / 对象时请求被静默丢弃

```bash
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":null,"method":"ping"}' \
 '{"jsonrpc":"2.0","id":1.5,"method":"ping"}' \
 '{"jsonrpc":"2.0","id":{"a":1},"method":"ping"}' | ddc mcp stdio
# -> 只有 initialize 的响应；三个 ping 无任何响应
```

**期望**：JSON-RPC 2.0 允许 String/Number/NULL id。`null`/数字应原样回包；对象 id 应返回 `-32600 Invalid request`（id 为 null），而不是无响应。静默丢弃会让客户端永久等待。

### 5. `tools/call` 缺少 `params` 时返回 `-32601 Method not found`

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":5,"method":"tools/call"}' | ddc mcp stdio
# -> {"error":{"code":-32601,"message":"tools/call"}}
```

**期望**：`-32602 Invalid params`（方法存在，参数缺失），而不是"方法不存在"。

### 6. HTTP body 畸形 JSON 返回 415（应 400/422）

```bash
curl -i -X POST 'http://127.0.0.1:8080/mcp' -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' -d '{"bad":'
# -> HTTP 415 "fail to deserialize request body EOF while parsing a value..."
curl ... -d '{"jsonrpc":"1.0","id":5,"method":"ping"}'
# -> HTTP 415
```

**期望**：`400`（或 `422`）并返回 JSON-RPC `-32700 Parse error` / `-32600 Invalid request`。415 表示媒体类型不受支持，与 Content-Type 正确的事实矛盾，客户端容易误判。

### 7. stdio 非 JSON 行被静默忽略，且与畸形 JSON 的处理不一致

```bash
printf 'garbage\n' | ddc mcp stdio                       # -> 无输出
printf 'not json at all {\n' | ddc mcp stdio             # -> -32600 Invalid request
```

**期望**：两者应一致；按 JSON-RPC 2.0 应返回 `-32700 Parse error`（以带 id:null 的响应或错误通知形式），或明确文档化为"丢弃非 JSON 行"（便于日志混入）。

### 8. `server/discover` 无 params 时返回 `-32601`，与其他缺参行为不一致

```bash
# --stateless，2026-07-28
curl ... -d '{"jsonrpc":"2.0","id":1,"method":"server/discover"}'
# -> -32601 "server/discover"
curl ... -d '{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}'
# -> -32602 缺少 _meta 字段（期望的行为）
```

**期望**：该方法存在，缺参应统一返回 `-32602` 并指出缺失的 `_meta` 字段，而不是"方法不存在"。

### 9. 会话 DELETE 非幂等可见（低优先级）

同一 `mcp-session-id` 连续 DELETE 两次 → 两次都 202；建议第二次返回 404，便于客户端确认会话已终止。

---

## P3. 已验证正常（修改上述代码时请勿回归）

- **版本门控**：`outputSchema` 仅在协商版本 ≥ 2025-06-18 时发射（2024-11-05 / 2025-03-26 → 0 个；2025-06-18 / 2025-11-25 / 2026-07-28 → 4 个 object schema）；`--versions` 钉定同时约束握手版本。
- **2026-07-28 stateless**：请求需要 `MCP-Protocol-Version` + `Mcp-Method`（`tools/call` 还需 `Mcp-Name`）头与请求 `_meta`（`io.modelcontextprotocol/protocolVersion`、`clientCapabilities`）；合规请求下 `server/discover`、`tools/list`、`tools/call`、`--json-response` 均正常。
- **会话生命周期**：创建/隔离/`--session-timeout` 过期 404/删除后 404/无会话 422；并发 20 调用无错；GET SSE 返回 `retry` 帧。
- **serve `/mcp` 身份继承**（0.4.10 修复）：stdin/http/serve 三处 serverInfo 一致。
- **表格渲染**（0.4.11 修复）：单元格宽度按字符计、补白封顶 200；>65535 字符单元格不再触发 `Formatting argument out of range` panic（`strings --with-locations` 的 `""` 条目曾打死 MCP worker）；请保留这两个回归测试。
- **MCPFieldHint.default**：inputSchema 默认值与 MCP 通道调用默认值一致（ddc 用它给四个列表工具设 limit=500），工作正常。
- **结果一致性**：同一调用在 MCP stdio 与 MCP http 的文本逐字节一致；REST 的 GET/POST 对全部 10 个路由响应逐字节一致。

---

## 附：复现环境

```
xyz-rust 0.4.11
rmcp 3.5.1
ddc 0.1.31（GitHub ejfkdev/ddc）
测试 APK：37 个 dex、213,523 个类
```

ddc 侧对应 issue：<https://github.com/ejfkdev/ddc/issues/7>（分页/契约）。
