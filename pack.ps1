# PlotVault PDM - 一键打包脚本
# 用法：项目根目录执行  .\pack.ps1
# 输出：plotvault-pdm-handover.zip（自动排除构建产物/依赖/临时文件）

param(
    [string]$Out = "plotvault-pdm-handover.zip"
)

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$stage = Join-Path $env:TEMP "plotvault-pdm-pack"
$dest = Join-Path $root $Out

Write-Host "=== cleaning ===" -ForegroundColor Cyan
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $dest) { Remove-Item -Force $dest }

Write-Host "=== copying sources (excluding build artifacts) ===" -ForegroundColor Cyan
robocopy $root $stage /E /XD node_modules target dist gen data .git /XF .env *.zip /NFL /NDL /NJH /NJS /NP
if ($LASTEXITCODE -gt 7) { throw "robocopy failed, exit code $LASTEXITCODE" }

Write-Host "=== zipping ===" -ForegroundColor Cyan
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $dest -CompressionLevel Optimal
Remove-Item -Recurse -Force $stage

$size = [Math]::Round((Get-Item $dest).Length / 1MB, 2)
Write-Host ""
Write-Host "Done! $dest (${size} MB)" -ForegroundColor Green
Write-Host "On new PC:  cd plotvault-pdm;  .\build-push.ps1"
