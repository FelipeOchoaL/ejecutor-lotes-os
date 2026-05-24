# Envia un mensaje JSON al pipe indicado y muestra la respuesta.
#
# Uso:
#   .\scripts\send.ps1 -Pipe pipe_ctrllt -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'

param(
    [string]$Pipe = "pipe_ctrllt",
    [Parameter(Mandatory = $true)][string]$Mensaje
)

$p  = New-Object System.IO.Pipes.NamedPipeClientStream('.', $Pipe, [System.IO.Pipes.PipeDirection]::InOut)
$p.Connect(3000)
$sw = New-Object System.IO.StreamWriter($p); $sw.AutoFlush = $true
$sr = New-Object System.IO.StreamReader($p)
$sw.WriteLine($Mensaje)
$resp = $sr.ReadLine()
$p.Close()
Write-Output $resp
