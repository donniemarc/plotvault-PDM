# PlotVault PDM - Build server image and push to Docker Hub
# Usage:
#   1. Install and start Docker Desktop (or any Docker daemon) on your PC
#   2. Run this script in PowerShell
#   3. Enter your Docker Hub username when prompted (docker login on first run)

param(
    [string]$User = "",
    [string]$Tag = "latest"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot   # plotvault-pdm project root

if (-not $User) {
    $User = Read-Host "Docker Hub username"
}
if (-not $User) {
    throw "Docker Hub username is required"
}

$image = "$User/plotvault-pdm-server:$Tag"

Write-Host "=== docker login ===" -ForegroundColor Cyan
docker login
if ($LASTEXITCODE -ne 0) { throw "docker login failed" }

Write-Host "=== Building image: $image ===" -ForegroundColor Cyan
docker build -t $image (Join-Path $root "server")
if ($LASTEXITCODE -ne 0) { throw "build failed" }

Write-Host "=== Pushing: $image ===" -ForegroundColor Cyan
docker push $image
if ($LASTEXITCODE -ne 0) { throw "push failed" }

Write-Host ""
Write-Host "Done! Image pushed to $image" -ForegroundColor Green
Write-Host "Next: deploy it on any Docker host with docker-compose.yml (replace the image tag with yours)."
