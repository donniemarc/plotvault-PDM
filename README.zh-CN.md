# PlotVault PDM（中文说明）

轻量级**自建图文档管理系统**。后端可部署在任何运行 Docker 的机器上（NAS、家庭服务器、VPS 或本机），Windows 原生桌面客户端连接管理。

- **后端**：Rust + Axum + **PostgreSQL**，单个静态二进制，镜像约 20MB，7×24 常驻零负担
- **客户端**：Tauri 2 + Vue 3 + Three.js，原生窗口、安装包小、启动快
- **存储**：图纸文件直接以真实目录存在磁盘上（目录即数据，备份就是拷贝目录），元数据与版本历史存 PostgreSQL；软件缓存（`/config`）与图纸数据（`/data`）分开挂载

> 本文件是 README 的中文翻译版，英文原版见 [README.md](README.md)。

## 功能特性

### 基础
- 文件夹树 + 文件列表
- 上传（拖拽）、下载、删除、重命名、移动
- **版本管理**：同名文件重复上传自动成为新版本，可查看历史、下载任意旧版、附版本备注
- 预览（见下表）
- 文件名搜索
- 可选 API Token（局域网访问保护）

### 预览支持矩阵
| 格式 | 方案 |
|---|---|
| DWG | 服务端 `libredwg` 转 DXF → 客户端 Three.js 渲染 |
| DXF | 客户端直接渲染（LINE/多段线/圆/圆弧/椭圆/样条/文字标注） |
| STEP / STP / IGES / IGS | 客户端 `occt-import-js`（OpenCascade WASM）直接解析，完整曲面/装配体 |
| STL | Three.js 直接渲染 |
| PDF | pdf.js 渲染 |
| 图片 / 文本 | 内建视图 |

所有 2D/3D 预览都支持鼠标旋转、缩放、平移。

## 目录结构

```
plotvault-pdm/
├── server/                 # Rust 后端
│   ├── src/                #   main.rs / db.rs / api.rs / storage.rs / convert.rs
│   ├── Dockerfile          #   多阶段构建，含 libredwg（DWG→DXF）
│   └── Cargo.toml
├── client/                 # Tauri + Vue3 桌面客户端
│   ├── src/                #   Vue 前端（UI + 预览）
│   └── src-tauri/          #   Tauri 壳
├── docker-compose.yml       # 一键部署（本地构建或拉取镜像）
├── build-push.ps1           # Windows 一键构建 + 推送到 Docker Hub
├── .env.example
└── README.md
```

## 一、部署后端

### 方式 A：Docker Compose（最简单）

在任意装有 Docker 的机器上：

```bash
# 1. 建一个目录，把仓库里的 docker-compose.yml 和 .env.example 放进去

cp .env.example .env        # 设置 API_TOKEN / POSTGRES_PASSWORD / PORT

# 2. 若想用 Docker Hub 上预构建的镜像：
#    把 docker-compose.yml 的 image 行改成 <你的用户名>/plotvault-pdm-server:latest

docker compose up -d --build   # 若用已拉取的镜像则不加 --build
```

一次启动三个容器：
- `db` — PostgreSQL 18（元数据）
- `plotvault-pdm` — 后端 API，端口 8642
- `converter`（可选）— STEP/IGES 服务端转换，端口 8000

### 方式 B：构建镜像并推送 Docker Hub（用于远程主机）

在 Windows PC 上（需 Docker Desktop）：

```powershell
.\build-push.ps1        # 按提示输入 Docker Hub 用户名
```

然后在目标主机上把 `docker-compose.yml` 的镜像指向 `<你的用户名>/plotvault-pdm-server:latest`，执行 `docker compose up -d`。

### 方式 C：离线 tar 导入（无外网环境）

```bash
# PC 上：
docker build -t plotvault-pdm-server ./server
docker save plotvault-pdm-server | gzip > plotvault-pdm-server.tar.gz
# 传送到目标主机后：
docker load < plotvault-pdm-server.tar.gz
docker compose up -d    # 使用已加载本地镜像的 compose 文件
```

### 验证部署

```bash
curl http://<主机IP>:8642/api/health
# {"service":"plotvault-pdm","status":"ok"}
```

## 二、构建 Windows 客户端

在一台 Windows PC 上构建一次即可（安装包可分发给其他机器）。

需要：Rust（MSVC 工具链 + Visual Studio Build Tools C++ 工作负载）、Node.js 18+、WebView2 运行时（Win10/11 一般自带）。

```powershell
cd client
npm install
npm run tauri build        # 生成 NSIS/MSI 安装包
# 产物在 client/src-tauri/target/release/bundle/
```

开发调试：

```powershell
cd client
npm run tauri dev
```

## 三、使用说明

1. 启动客户端 → **设置** → 填入服务器地址（`http://<主机IP>:8642`）和 API Token → 测试连接 → 保存。
2. **文件**页浏览文件夹树，单击文件预览，按钮执行下载/版本/重命名/移动/删除。
3. 上传：右上角「上传」或直接把文件拖进窗口。同目录同名文件自动作为新版本。
4. 版本：选中文件点「版本」，可查看历史、下载旧版、上传新版本并附备注。
5. 预览右上角可关闭。

## 四、配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `DATA_DIR` | `/data` | 图纸数据目录（library 真实目录 + blobs 版本归档），主机上可直接浏览 |
| `CONFIG_DIR` | `/config` | 软件缓存目录（dxf_cache + tmp） |
| `DATABASE_URL` | `postgres://postgres:postgres@localhost:5432/plotvault_pdm` | PostgreSQL 连接串（compose 里指向 `db` 容器） |
| `BIND` | `0.0.0.0:8642` | 监听地址 |
| `API_TOKEN` | 空 | 设置后所有 API 需 `Authorization: Bearer <token>` |

> ⚠️ 即使是局域网私有环境，也建议设置 `API_TOKEN`。

## 五、FAQ

**DWG 预览报错「dwg2dxf not available」？**
说明容器内没有 `dwg2dxf`（本机裸跑二进制时常见，Docker 镜像会从源码编译 libredwg）。请用 Docker 部署；转换只在首次预览时执行并缓存到 `CONFIG_DIR/dxf_cache`。

**如何备份？**
备份图纸拷贝 `data/` 目录（library 真实目录 + blobs 归档）即可；建议一并备份 `pgdata/` 卷（目录结构、版本备注等元数据，丢了不影响图纸文件本身）。

**想清空重建？**
删除 `config/` + 清空数据库即可重置元数据；删除 `data/` 则连图纸一起清空。

**数据库连不上？**
确认 compose 中 `db` 容器先启动且健康（`pg_isready`），`DATABASE_URL` 的用户/密码/库名与 `POSTGRES_*` 一致。

## 后续可扩展方向
- 缩略图（STL/STEP 服务端渲染小图）
- 版本对比 / DWG 导出 PDF
- 批量打包下载（zip）
- 标签 / 分类 / 全文检索
- 审计日志、多用户权限

## 许可

[MIT License](LICENSE) © 2026 donniemarc
