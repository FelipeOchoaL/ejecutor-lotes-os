# Arranca los cuatro servicios del sistema ctrllt en Windows.
#
# Uso desde la raíz del proyecto:
#   .\scripts\start.ps1
#
# Tras esto se puede usar un cliente que envíe peticiones JSON al pipe
# llamado "pipe_ctrllt" (\\.\pipe\pipe_ctrllt).

param(
    [string]$Aralmac = "aralmac",
    [string]$PipeCtrllt   = "pipe_ctrllt",
    [string]$PipeGesfich  = "pipe_gesfich",
    [string]$PipeGesprog  = "pipe_gesprog",
    [string]$PipeEjecutor = "pipe_ejecutor",
    [switch]$Release
)

$ErrorActionPreference = "Stop"

$target = if ($Release) { "release" } else { "debug" }
$bin    = Join-Path "target" $target

if (-not (Test-Path "$bin\ctrllt.exe")) {
    Write-Host "Compilando..."
    if ($Release) { cargo build --release } else { cargo build }
}

if (-not (Test-Path $Aralmac)) { New-Item -ItemType Directory -Path $Aralmac | Out-Null }

Write-Host "Arrancando servicios (aralmac=$Aralmac)..."

Start-Process -FilePath "$bin\gesfich.exe"  -ArgumentList "-f",$PipeGesfich, "-x",$Aralmac -WindowStyle Hidden
Start-Process -FilePath "$bin\gesprog.exe"  -ArgumentList "-p",$PipeGesprog, "-x",$Aralmac -WindowStyle Hidden
Start-Process -FilePath "$bin\ejecutor.exe" -ArgumentList "-e",$PipeEjecutor,"-x",$Aralmac -WindowStyle Hidden
Start-Sleep -Milliseconds 600
Start-Process -FilePath "$bin\ctrllt.exe"   -ArgumentList "-c",$PipeCtrllt, "-f",$PipeGesfich,"-p",$PipeGesprog,"-e",$PipeEjecutor -WindowStyle Hidden

Write-Host "Sistema listo. Pipe del cliente: \\.\pipe\$PipeCtrllt"
