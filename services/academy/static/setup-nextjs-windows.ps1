#Requires -RunAsAdministrator
# Idempotent Next.js dev environment setup for Windows: git, nvm-windows, Node LTS, pnpm/yarn.
# Safe to re-run. Run from an admin PowerShell:
#   Set-ExecutionPolicy Bypass -Scope Process -Force
#   .\setup-nextjs-windows.ps1

$ErrorActionPreference = "Stop"

function Test-Cmd($name) { [bool](Get-Command $name -ErrorAction SilentlyContinue) }

function Refresh-Path {
  # winget/nvm installers write PATH to the registry but don't touch this
  # process's environment, so pull Machine + User PATH back in right away.
  $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $user    = [Environment]::GetEnvironmentVariable("Path", "User")
  $env:Path = "$machine;$user"
}

if (-not (Test-Cmd git)) {
  Write-Host "Installing Git..."
  winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
  Refresh-Path
}

if (-not (Test-Cmd nvm)) {
  Write-Host "Installing nvm-windows..."
  winget install --id CoreyButler.NVMforWindows -e --source winget --accept-package-agreements --accept-source-agreements
  Refresh-Path
}

if (-not (Test-Cmd nvm)) {
  Write-Host "nvm installed but this shell can't see it yet. Close this window, reopen PowerShell as Administrator, and re-run this script."
  exit 0
}

$hasNode = Test-Cmd node
$nodeMajor = if ($hasNode) { [int]((node -v).TrimStart('v').Split('.')[0]) } else { 0 }

if ($hasNode -and $nodeMajor -ge 18) {
  Write-Host "Node $(node -v) already installed, leaving it alone."
} else {
  nvm install lts
  nvm use lts
  Refresh-Path
}

corepack enable
corepack prepare pnpm@latest --activate
corepack prepare yarn@stable --activate   # ponytail: delete this block if you don't want yarn

# --- Self-check: everything must resolve on PATH right now, in this same shell ---
Write-Host ""
Write-Host "Checking PATH..."
$ok = $true
foreach ($cmd in @("git --version", "node -v", "corepack -v", "pnpm -v", "yarn -v")) {
  try {
    $out = Invoke-Expression $cmd 2>&1
    Write-Host "  OK   $cmd -> $out"
  } catch {
    Write-Host "  FAIL $cmd -> $_"
    $ok = $false
  }
}

if ($ok) {
  Write-Host "Done. Everything is on PATH."
} else {
  Write-Host "Some tools aren't resolving yet in this shell. Open a brand new admin PowerShell and re-run this script."
  exit 1
}
