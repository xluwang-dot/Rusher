# Rusher 🚀

GitHub DNS 加速服务 - Rust 实现

## 功能特性

- ✅ 使用 GitHub 公开的 IP 范围，扫描所有可用的 IP
- ✅ 间隔指定时间检测与记录扫描到的 IP 的访问耗时
- ✅ 拦截 DNS，访问 GitHub 时返回最快的 IP
- ✅ 支持 IPv4 和 IPv6
- ✅ 支持 DNS over HTTPS
- ✅ 高性能异步实现
- ✅ 完善的配置管理
- ✅ 结构化日志系统
- ✅ 优雅关机支持
- ✅ 监控和健康检查

## 快速开始

### 安装

```bash
# 克隆项目
git clone https://github.com/yourusername/rusher.git
cd rusher

# 构建项目
cargo build --release

# 安装到系统
cargo install --path .
```

### 基本使用

```bash
# 生成默认配置
rusher generate

# 测试配置
rusher test

# 启动服务
rusher start

# 查看版本
rusher version

# 查看帮助
rusher help
```

### 配置文件

配置文件支持多种格式（TOML、JSON、YAML），默认使用 TOML 格式。

默认配置文件位置：
- `./config/default.toml`
- `./config/development.toml`


## 配置说明

### 通用配置
```toml
[general]
log_level = "info"           # 日志级别: trace, debug, info, warn, error
log_file = "rusher.log"  # 日志文件路径，留空则输出到控制台
daemon = false               # 是否以守护进程模式运行
```

### DNS 配置
```toml
[dns]
listen_addr = "0.0.0.0:53"   # DNS 监听地址
cache_ttl = 300              # DNS 缓存生存时间（秒）
enable_ipv6 = false          # 是否启用 IPv6
upstream_dns = ["8.8.8.8:53", "1.1.1.1:53"]  # 上游 DNS 服务器
doh_enabled = false          # 是否启用 DNS over HTTPS
doh_endpoint = "https://cloudflare-dns.com/dns-query"  # DoH 端点
```

### 扫描器配置
```toml
[scanner]
scan_interval = 300          # 全量扫描间隔（秒）
scan_timeout = 10            # 扫描超时时间（秒）
max_concurrent = 100         # 最大并发连接数
retry_count = 3              # 重试次数
connect_timeout = 5          # 连接超时（秒）
request_timeout = 10         # 请求超时（秒）
incremental_scan = true      # 是否启用增量扫描
incremental_interval = 60    # 增量扫描间隔（秒）
```

### GitHub 配置
```toml
[github]
meta_url = "https://api.github.com/meta"  # GitHub Meta API 地址
domains = [                               # 需要加速的 GitHub 域名
    "github.com",
    "github.global.ssl.fastly.net",
    "raw.githubusercontent.com",
    "gist.github.com",
    "github.io",
    "githubusercontent.com",
    "githubassets.com",
    "github.dev"
]
custom_ranges = []                        # 自定义 IP 范围（CIDR格式）
api_auth_enabled = false                  # 是否启用 GitHub API 认证
api_token = ""                            # GitHub API Token
```

## 开发指南

### 项目结构
```
rusher/
├── Cargo.toml            # 项目配置和依赖
├── config/               # 配置文件
│   ├── default.toml     # 默认配置
│   └── development.toml # 开发环境配置
├── src/                 # 源代码
│   ├── main.rs         # 程序入口
│   ├── lib.rs          # 库入口
│   ├── error.rs        # 错误处理
│   ├── config/         # 配置管理
│   │   ├── mod.rs
│   │   ├── models.rs   # 配置数据结构
│   │   └── loader.rs   # 配置加载器
│   ├── scanner/        # IP 扫描模块
│   ├── dns/            # DNS 服务模块
│   ├── http/           # HTTP 客户端
│   ├── github/         # GitHub 相关功能
│   └── utils/          # 工具函数
│       ├── mod.rs
│       ├── logging.rs  # 日志系统
│       └── signal.rs   # 信号处理
└── tests/              # 测试代码
    ├── integration/    # 集成测试
    └── unit/          # 单元测试
```

### 开发环境设置

```bash
# 安装 Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 克隆项目
git clone https://github.com/yourusername/rusher.git
cd rusher

# 安装开发依赖
cargo install cargo-watch  # 文件变化监控
cargo install cargo-clippy # 代码检查工具

# 运行开发服务器
cargo run -- --config config/development.toml

# 运行测试
cargo test

# 代码检查
cargo clippy

# 代码格式化
cargo fmt
```

### 添加新功能

1. 在相应的模块目录中创建新的 Rust 文件
2. 在模块的 `mod.rs` 中导出新模块
3. 实现功能逻辑
4. 添加单元测试
5. 更新文档

## 部署指南

### 系统服务（Systemd）

创建服务文件 `/etc/systemd/system/rusher.service`：

```ini
[Unit]
Description=Rusher DNS Accelerator
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/rusher
ExecStart=/usr/local/bin/rusher start --config /etc/rusher/config.toml
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

启用并启动服务：
```bash
sudo systemctl daemon-reload
sudo systemctl enable rusher
sudo systemctl start rusher
sudo systemctl status rusher



构建和运行：
```bash
docker build -t rusher .
docker run -d --name rusher -p 53:53/udp -p 53:53/tcp rusher
```

## 性能优化

### 编译优化

在 `Cargo.toml` 中启用发布优化：
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
```

### 运行时优化

1. **调整并发数**：根据服务器 CPU 核心数调整 `max_concurrent`
2. **优化缓存大小**：根据内存大小调整缓存配置
3. **启用 HTTP/2**：减少连接建立开销
4. **使用连接池**：复用 HTTP 连接

## 故障排除

### 常见问题

1. **权限问题**：DNS 服务需要绑定 53 端口，需要 root 权限或 CAP_NET_BIND_SERVICE 能力
2. **端口冲突**：确保 53 端口没有被其他服务占用
3. **网络问题**：检查防火墙设置，确保可以访问 GitHub API
4. **配置错误**：使用 `rusher test` 命令测试配置

### 日志查看

```bash
# 查看实时日志
tail -f rusher.log

# 根据日志级别过滤
grep "ERROR" rusher.log
grep "WARN" rusher.log

# 查看最近错误
tail -100 rusher.log | grep -E "(ERROR|WARN)"
```

### 性能监控

启用监控功能：
```toml
[monitoring]
enabled = true
port = 9090
path = "/metrics"
health_check = true
```

访问监控端点：
```bash
curl http://localhost:9090/metrics
curl http://localhost:9090/health
```

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

本项目采用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

## 致谢

- 感谢原 .NET 版本 [FastGithub](https://github.com/dotnetcore/FastGithub) 的启发
- 感谢所有贡献者和用户的支持

## 联系方式

- 项目主页：https://github.com/xluwang-dot/rusher
- 问题反馈：https://github.com/xluwang-dot/rusher/issues
- 讨论区：https://github.com/xluwang-dot/rusher/discussions
