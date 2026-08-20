# 图文档管理 - PC 一键构建 + 推送到 Docker Hub
# 用法：
#   1. 安装 Docker Desktop 并启动（Windows 上装 Docker Desktop 不涉及 NAS）
#   2. 在 PowerShell 里运行本脚本
#   3. 按提示输入 Docker Hub 用户名（首次会要求 docker login）

param(
    [string]$User = "",
    [string]$Tag = "latest"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot   # truendang 项目根目录

if (-not $User) {
    $User = Read-Host "Docker Hub 用户名"
}
if (-not $User) {
    throw "必须提供 Docker Hub 用户名"
}

$image = "$User/tuwendang-server:$Tag"

Write-Host "=== 登录 Docker Hub ===" -ForegroundColor Cyan
docker login
if ($LASTEXITCODE -ne 0) { throw "docker login 失败" }

Write-Host "=== 构建 linux/amd64 镜像: $image ===" -ForegroundColor Cyan
docker build -t $image (Join-Path $root "server")
if ($LASTEXITCODE -ne 0) { throw "构建失败" }

Write-Host "=== 推送: $image ===" -ForegroundColor Cyan
docker push $image
if ($LASTEXITCODE -ne 0) { throw "推送失败" }

Write-Host ""
Write-Host "完成！镜像已推送到 $image" -ForegroundColor Green
Write-Host "接下来：TrueNAS Web UI -> Apps -> Discover -> Custom App -> 粘贴 truenas-compose.yml"
Write-Host "把 image 行的 <DOCKERHUB_USER> 换成你的用户名，填好 API_TOKEN 和数据路径后部署。"
