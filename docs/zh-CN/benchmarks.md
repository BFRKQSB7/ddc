# 基准测试

[English](../benchmarks.md) | [简体中文]

七个真实 APK —— reqable 3.2.23（34MB）、Telegram（62MB）、WhatsApp
（139MB）、weibo 16.9.1（226MB、20 dex）、weixin 8.0.78（268MB）、
飞书 8.0.2（398MB、59 dex）、qq 9.3.65（374MB）—— 墙钟时间冷机
后实测，峰值 RSS 来自 `/usr/bin/time -l`：

| APK | 大小 | 全量反编译 | `--fast` | 峰值 RSS |
|---|---|---|---|---|
| reqable | 34 MB | **0.83s** | 0.50s | 280 MB |
| Telegram | 62 MB | **6.91s** | 4.08s | 924 MB |
| WhatsApp | 139 MB | **24.98s** | 11.41s | 1067 MB |
| weibo | 226 MB | **21.03s** | 10.87s | 1380 MB |
| weixin | 268 MB | **40.21s** | 25.29s | 1497 MB |
| lark | 398 MB | **26.29s** | 14.61s | 2406 MB |
| qq | 374 MB | **57.00s** | 31.96s | 2511 MB |

**编译验证——引用这个数字前请先读口径**：七个 APK 的全部 `.java`（共
909,689 个文件）过 `javac` 解析闸门（`-XDshould-stop.ifNoError=PARSE
-XDshould-stop.ifError=PARSE`）——**语法错误为零**。该闸门只到解析器为止：
**不做类型检查、不做符号解析、不保证挂 classpath 后能编译通过**。

**语义质量另有独立度量——带完整 Android classpath（android-37 jar +
可选库）的 javac 电池**，覆盖四个语料，只计 ddc 侧可归因诊断。该电池
是 0.1.4 之后多轮修复的驱动力，**v0.1.19 → v0.1.22 从 11,345 降到
7,452（−34%）**——其中包括一个自首个版本就存在的多 handler 异常路由
bug（多 handler 方法的所有 catch 全部执行第一个 handler 的代码——任何
解析闸门都不可见）、以及三个靠逐行读输出对照反汇编才发现的 javac 不可
见值丢失缺陷。语义质量在类型恢复上仍落后 jadx（擦除泛型、R8 巨兽方法
的寄存器类型局部）——见 README「已知限制」。

记录于 2026-10-22，v0.1.33：每个单元格一次运行（短暂静置后）；旧表的“全量”列混入了 `--fast` 数据。峰值 RSS 来自 `/usr/bin/time -l`。

## 同一批 APK 上的全部查询子命令

冷机后实测；单元格为 墙钟 / 峰值 RSS。查询内容：`strings -f <包名>
--with-locations`；`findrefs string <包名>`；`findrefs method onCreate`；
`hierarchy`/`disasm`/`getclass` 用各 App 的启动类（Telegram 的
`LaunchActivity` 是特别大的类）：

| 子命令 | reqable | Telegram | WhatsApp | weibo | weixin | lark | qq |
|---|---|---|---|---|---|---|---|
| `info` | 0.24s / 55MB | 0.41s / 117MB | 0.85s / 316MB | 1.32s / 569MB | 1.61s / 602MB | 2.46s / 969MB | 2.30s / 1145MB |
| `listclasses` | 0.02s / 25MB | 0.03s / 89MB | 0.04s / 96MB | 0.14s / 337MB | 0.16s / 371MB | 0.18s / 251MB | 0.24s / 311MB |
| `manifest` | 0.00s / 12MB | 0.00s / 19MB | 0.00s / 26MB | 0.01s / 48MB | 0.01s / 37MB | 0.01s / 34MB | 0.01s / 54MB |
| `mainactivity` | 0.02s / 20MB | 0.06s / 72MB | 0.08s / 136MB | 0.17s / 199MB | 0.18s / 199MB | 0.22s / 226MB | 0.31s / 300MB |
| `res` | 0.00s / 12MB | 0.00s / 20MB | 0.00s / 27MB | 0.02s / 56MB | 0.01s / 37MB | 0.01s / 36MB | 0.01s / 57MB |
| `largest` | 0.02s / 28MB | 0.05s / 71MB | 0.13s / 195MB | 0.32s / 363MB | 0.30s / 346MB | 0.46s / 519MB | 0.66s / 631MB |
| `strings` | 0.02s / 20MB | 0.04s / 55MB | 0.12s / 117MB | 0.23s / 177MB | 0.28s / 198MB | 0.18s / 192MB | 0.49s / 295MB |
| `findrefs-string` | 0.02s / 31MB | 0.03s / 74MB | 0.04s / 203MB | 0.06s / 397MB | 0.08s / 425MB | 0.04s / 322MB | 0.10s / 849MB |
| `findrefs-method` | 0.02s / 31MB | 0.05s / 81MB | 0.04s / 212MB | 0.06s / 374MB | 0.07s / 448MB | 0.07s / 581MB | 0.11s / 917MB |
| `members` | 0.01s / 19MB | 0.03s / 55MB | 0.05s / 118MB | 0.11s / 174MB | 0.11s / 180MB | 0.14s / 218MB | 0.21s / 285MB |
| `hierarchy` | 0.01s / 20MB | 0.03s / 55MB | 0.05s / 127MB | 0.10s / 174MB | 0.11s / 177MB | 0.15s / 194MB | 0.21s / 294MB |
| `disasm` | 0.01s / 19MB | 0.03s / 58MB | 0.05s / 124MB | 0.10s / 175MB | 0.11s / 182MB | 0.14s / 193MB | 0.20s / 281MB |
| `getclass` | 0.02s / 33MB | 0.21s / 131MB | 0.18s / 256MB | 0.41s / 515MB | 0.50s / 527MB | 0.70s / 830MB | 1.30s / 1200MB |

测量方法论（冷机实测、串行纪律、CPU 计量陷阱）见
[英文版](../benchmarks.md) 的 Methodology notes 与
[性能工程](optimization.md)。
