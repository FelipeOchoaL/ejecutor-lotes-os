# Guía de ejecución — Sistema `ctrllt`

Guía rápida con **todos los comandos a mano** para arrancar, probar y apagar
el sistema en **Windows** y **Linux**, junto con ejemplos completos por
escenario.

> Las referencias al PDF tienen la forma **(PDF §3.X)**.

---

## Tabla de contenidos

1. [Compilar](#1-compilar)
2. [Arrancar el sistema](#2-arrancar-el-sistema)
3. [Enviar mensajes (cliente de pruebas)](#3-enviar-mensajes-cliente-de-pruebas)
4. [Catálogo de mensajes JSON por servicio](#4-catálogo-de-mensajes-json-por-servicio)
5. [Errores estándar del PDF](#5-errores-estándar-del-pdf)
6. [Apagar el sistema](#6-apagar-el-sistema)
7. [Guías de ejecución por escenario (ejemplos)](#7-guías-de-ejecución-por-escenario-ejemplos)
8. [Equivalencias Windows ↔ Linux](#8-equivalencias-windows--linux)
9. [Solución de problemas](#9-solución-de-problemas)

---

## 1. Compilar

### Windows / PowerShell

```powershell
cd practica
cargo build               # debug (target\debug\*.exe)
cargo build --release     # release (target\release\*.exe)
```

### Linux / bash

```bash
cd practica
cargo build               # debug  (target/debug/*)
cargo build --release     # release (target/release/*)
```

Los cuatro binarios se llaman `ctrllt`, `gesfich`, `gesprog`, `ejecutor`
(con `.exe` en Windows).

---

## 2. Arrancar el sistema

### 2.1. Arranque automático (recomendado)

#### Windows

```powershell
cd practica
.\scripts\start.ps1
# Variantes:
.\scripts\start.ps1 -Release
.\scripts\start.ps1 -Aralmac C:\datos\aralmac
.\scripts\start.ps1 -PipeCtrllt mi_pipe
```

Tras arrancar, el cliente se conecta a `\\.\pipe\pipe_ctrllt`.

#### Linux

```bash
cd practica
chmod +x scripts/start.sh
./scripts/start.sh
# Variantes (mediante variables de entorno):
PROFILE=release ./scripts/start.sh
ARALMAC=/tmp/aralmac ./scripts/start.sh
PIPE_CTRLLT=mi_pipe ./scripts/start.sh
```

Tras arrancar, el cliente se conecta al socket abstracto `@pipe_ctrllt`.

### 2.2. Arranque manual

#### Windows / PowerShell

```powershell
mkdir aralmac -Force | Out-Null

Start-Process .\target\debug\gesfich.exe  -ArgumentList '-f','pipe_gesfich', '-x','aralmac' -WindowStyle Hidden
Start-Process .\target\debug\gesprog.exe  -ArgumentList '-p','pipe_gesprog', '-x','aralmac' -WindowStyle Hidden
Start-Process .\target\debug\ejecutor.exe -ArgumentList '-e','pipe_ejecutor','-x','aralmac' -WindowStyle Hidden
Start-Sleep -Milliseconds 500
Start-Process .\target\debug\ctrllt.exe   -ArgumentList '-c','pipe_ctrllt','-f','pipe_gesfich','-p','pipe_gesprog','-e','pipe_ejecutor' -WindowStyle Hidden
```

Verificar:

```powershell
Get-Process gesfich, gesprog, ejecutor, ctrllt | Format-Table Id, ProcessName
```

#### Linux / bash

```bash
mkdir -p aralmac

./target/debug/gesfich  -f pipe_gesfich  -x aralmac &
./target/debug/gesprog  -p pipe_gesprog  -x aralmac &
./target/debug/ejecutor -e pipe_ejecutor -x aralmac &
sleep 0.5
./target/debug/ctrllt -c pipe_ctrllt -f pipe_gesfich -p pipe_gesprog -e pipe_ejecutor &
```

Verificar:

```bash
ps -ef | grep -E 'ctrllt|gesfich|gesprog|ejecutor' | grep -v grep
ss -lx | grep pipe_   # ver sockets abstractos
```

### 2.3. Sinopsis del PDF

```
ctrllt   -c <pipe_cliente>  -f <pipe_gesfich>  -p <pipe_gesprog>  -e <pipe_ejecutor>
gesfich  -f <pipe_gesfich>  -x <dir_aralmac>
gesprog  -p <pipe_gesprog>  -x <dir_aralmac>
ejecutor -e <pipe_ejecutor> -x <dir_aralmac>
```

Las opciones `-a`, `-b`, `--resp-gesprog`, `-d` (tuberías de respuesta de
half-duplex del PDF §3.1) se aceptan pero no se usan: la IPC es full-duplex.

---

## 3. Enviar mensajes (cliente de pruebas)

### 3.1. Windows — `send.ps1`

```powershell
.\scripts\send.ps1 -Mensaje '<JSON>'
.\scripts\send.ps1 -Pipe pipe_gesfich -Mensaje '<JSON>'   # bypass del ctrllt
```

- `-Pipe` → opcional, default `pipe_ctrllt`.
- `-Mensaje` → obligatorio, el JSON de la petición.

### 3.2. Linux — función bash `send`

Pega esto en tu terminal **una sola vez por sesión**:

```bash
send() { echo "$1" | socat -t2 - "ABSTRACT-CONNECT:${2:-pipe_ctrllt}"; }
```

Uso:

```bash
send '<JSON>'
send '<JSON>' pipe_gesfich   # bypass del ctrllt
```

Alternativa con `ncat` si no tienes `socat` (requiere socket "real" en /tmp):

```bash
ncat -U /tmp/pipe_ctrllt.sock <<< '<JSON>'
```

> Si `socat` no soporta `ABSTRACT-CONNECT`, instala `socat` reciente:
> `sudo apt install socat` (Ubuntu/Debian) o `sudo dnf install socat` (Fedora).

### 3.3. ¿Qué pasa al enviar un mensaje? (resumen mental)

```
1) cliente abre la tubería pipe_ctrllt
2) escribe una línea JSON terminada en \n
3) ctrllt parsea, mira "servicio"
4) ctrllt reenvía a gesfich/gesprog/ejecutor (o maneja él mismo)
5) el servicio destino responde con otra línea JSON
6) ctrllt escribe esa respuesta literal al cliente
7) cliente lee, imprime, cierra
```

---

## 4. Catálogo de mensajes JSON por servicio

> Tamaño máximo por mensaje: **4096 bytes** (PDF §3.8.4).

### 4.1. `gesfich` (PDF §3.9)

| Operación | Petición |
|---|---|
| Crear | `{"servicio":"gesfich","operacion":"Crear"}` |
| Leer (uno) | `{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0001"}` |
| Leer (lista) | `{"servicio":"gesfich","operacion":"Leer"}` |
| Actualizar | `{"servicio":"gesfich","operacion":"Actualizar","id-fichero":"f-0001","ruta":"/ruta/al/archivo"}` |
| Borrar | `{"servicio":"gesfich","operacion":"Borrar","id-fichero":"f-0001"}` |
| Suspender | `{"servicio":"gesfich","operacion":"Suspender"}` |
| Reasumir | `{"servicio":"gesfich","operacion":"Reasumir"}` |
| Terminar | `{"servicio":"gesfich","operacion":"Terminar"}` |

Respuestas típicas:

```json
{"estado":"ok","id-fichero":"f-0001"}
{"estado":"ok","contenido":"<texto>"}
{"estado":"ok","ficheros":["f-0001","f-0002"]}
{"estado":"ok"}
{"estado":"error","mensaje":"fichero no encontrado"}
```

### 4.2. `gesprog` (PDF §3.10)

| Operación | Petición |
|---|---|
| Guardar | `{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/ruta/al/exe","args":["a"],"env":["K=V"]}` |
| Leer (uno) | `{"servicio":"gesprog","operacion":"Leer","id-programa":"p-0001"}` |
| Leer (lista) | `{"servicio":"gesprog","operacion":"Leer"}` |
| Actualizar | `{"servicio":"gesprog","operacion":"Actualizar","id-programa":"p-0001","ruta":"/nueva/ruta"}` |
| Borrar | `{"servicio":"gesprog","operacion":"Borrar","id-programa":"p-0001"}` |
| Suspender | `{"servicio":"gesprog","operacion":"Suspender"}` |
| Reasumir | `{"servicio":"gesprog","operacion":"Reasumir"}` |
| Terminar | `{"servicio":"gesprog","operacion":"Terminar"}` |

Respuestas típicas:

```json
{"estado":"ok","id-programa":"p-0001"}
{"estado":"ok","programa":{"id-programa":"p-0001","nombre":"sort","args":[],"env":[]}}
{"estado":"ok","programas":["p-0001","p-0002"]}
{"estado":"ok"}
{"estado":"error","mensaje":"falta campo: ejecutable"}
```

> **Caso especial:** `gesprog.Leer` está permitido aún en estado **Suspendido**
> (figura 4 del PDF). El resto de operaciones de datos no.

### 4.3. `ejecutor` (PDF §3.11)

| Operación | Petición |
|---|---|
| Ejecutar | `{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0001","stdin":"f-0001","stdout":"f-0002","stderr":"f-0003"}` |
| Estado (uno) | `{"servicio":"ejecutor","operacion":"Estado","id-ejecucion":"e-0001"}` |
| Estado (todos) | `{"servicio":"ejecutor","operacion":"Estado"}` |
| Matar | `{"servicio":"ejecutor","operacion":"Matar","id-ejecucion":"e-0001"}` |
| Suspender | `{"servicio":"ejecutor","operacion":"Suspender"}` |
| Reasumir | `{"servicio":"ejecutor","operacion":"Reasumir"}` |
| Parar | `{"servicio":"ejecutor","operacion":"Parar"}` |

Respuestas típicas:

```json
{"estado":"ok","id-ejecucion":"e-0001"}
{"estado":"ok","id-ejecucion":"e-0001","id-programa":"p-0001","proceso-estado":"Ejecutando"}
{"estado":"ok","id-ejecucion":"e-0001","id-programa":"p-0001","proceso-estado":"Terminado","codigo-salida":0}
{"estado":"ok","procesos":[ {...}, {...} ]}
{"estado":"error","mensaje":"proceso no encontrado o ya terminado"}
```

### 4.4. `ctrllt` (PDF §3.12)

| Operación | Petición |
|---|---|
| Terminar (apaga TODO) | `{"servicio":"ctrllt","operacion":"Terminar"}` |

---

## 5. Errores estándar del PDF

Mensajes literales que devuelve el sistema (útiles para validar la corrección):

| Servicio | Mensaje |
|---|---|
| `gesfich` | `no se pudo crear el fichero`, `fichero no encontrado`, `error al listar ficheros`, `faltan campos: id-fichero, ruta`, `no se pudo actualizar el fichero`, `transicion invalida`, `servicio suspendido`, `operacion desconocida` |
| `gesprog` | `falta campo: ejecutable`, `no se pudo guardar el programa`, `programa no encontrado`, `error al listar programas`, `faltan campos: id-programa, ruta`, `no se pudo actualizar el programa`, `transicion invalida`, `servicio suspendido`, `operacion desconocida` |
| `ejecutor` | `falta campo: id-programa`, `no se pudo ejecutar el programa`, `proceso no encontrado`, `falta campo: id-ejecucion`, `proceso no encontrado o ya terminado`, `transicion invalida`, `servicio suspendido`, `servicio parando`, `operacion desconocida` |
| `ctrllt` | `servicio desconocido`, `operacion ctrllt desconocida`, `servicio no conectado`, `error enviando solicitud al servicio`, `error leyendo respuesta del servicio` |

---

## 6. Apagar el sistema

### 6.1. Apagado limpio (recomendado)

```powershell
# Windows
.\scripts\send.ps1 -Mensaje '{"servicio":"ctrllt","operacion":"Terminar"}'
```

```bash
# Linux
send '{"servicio":"ctrllt","operacion":"Terminar"}'
```

Lo que hace internamente (PDF §3.12.1):

1. `ctrllt` envía `Terminar` a `gesfich` → muere.
2. `ctrllt` envía `Terminar` a `gesprog` → muere.
3. `ctrllt` envía `Parar` a `ejecutor` → no acepta nuevos, espera a sus hijos.
4. `ctrllt` responde `{"estado":"ok"}` y muere.
5. `ejecutor` muere cuando `procesos_activos == 0` (vía reaper).

### 6.2. Apagado de emergencia

```powershell
# Windows
Stop-Process -Name ctrllt, gesfich, gesprog, ejecutor -Force
```

```bash
# Linux
pkill -f 'target/debug/(ctrllt|gesfich|gesprog|ejecutor)'
```

---

## 7. Guías de ejecución por escenario (ejemplos)

### 7.1. Escenario A — CRUD completo de ficheros

#### Windows

```powershell
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'
# → {"estado":"ok","id-fichero":"f-0001"}

"hola mundo" | Out-File -Encoding ascii -NoNewline aralmac\hola.txt
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Actualizar","id-fichero":"f-0001","ruta":"aralmac\\hola.txt"}'
# → {"estado":"ok"}

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0001"}'
# → {"estado":"ok","contenido":"hola mundo"}

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Leer"}'
# → {"estado":"ok","ficheros":["f-0001"]}

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Borrar","id-fichero":"f-0001"}'
# → {"estado":"ok"}
```

#### Linux

```bash
send '{"servicio":"gesfich","operacion":"Crear"}'
echo "hola mundo" > /tmp/hola.txt
send '{"servicio":"gesfich","operacion":"Actualizar","id-fichero":"f-0001","ruta":"/tmp/hola.txt"}'
send '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0001"}'
send '{"servicio":"gesfich","operacion":"Leer"}'
send '{"servicio":"gesfich","operacion":"Borrar","id-fichero":"f-0001"}'
```

### 7.2. Escenario B — Ordenar números con `sort` (proceso de lotes)

#### Windows

```powershell
"5`n2`n8`n1`n3" | Out-File -Encoding ascii -NoNewline aralmac\input.txt

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'   # f-0001
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'   # f-0002

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Actualizar","id-fichero":"f-0001","ruta":"aralmac\\input.txt"}'

.\scripts\send.ps1 -Mensaje '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"C:\\Windows\\System32\\sort.exe"}'
# → p-0001

.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0001","stdin":"f-0001","stdout":"f-0002"}'
# → e-0001

.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Estado","id-ejecucion":"e-0001"}'
# → proceso-estado":"Terminado","codigo-salida":0

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0002"}'
# → {"estado":"ok","contenido":"1\r\n2\r\n3\r\n5\r\n8\r\n"}
```

#### Linux

```bash
echo -e "5\n2\n8\n1\n3" > /tmp/input.txt

send '{"servicio":"gesfich","operacion":"Crear"}'   # f-0001
send '{"servicio":"gesfich","operacion":"Crear"}'   # f-0002
send '{"servicio":"gesfich","operacion":"Actualizar","id-fichero":"f-0001","ruta":"/tmp/input.txt"}'

send '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/usr/bin/sort","args":["-n"],"env":["LC_ALL=C"]}'
# → p-0001

send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0001","stdin":"f-0001","stdout":"f-0002"}'
# → e-0001

send '{"servicio":"ejecutor","operacion":"Estado","id-ejecucion":"e-0001"}'
send '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0002"}'
# → {"estado":"ok","contenido":"1\n2\n3\n5\n8\n"}
```

### 7.3. Escenario C — Lanzar y matar un proceso largo

#### Windows

```powershell
.\scripts\send.ps1 -Mensaje '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"C:\\Windows\\System32\\timeout.exe","args":["/t","60","/nobreak"]}'
# → p-0002

.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0002"}'
# → e-0002

.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Estado","id-ejecucion":"e-0002"}'
# → proceso-estado":"Ejecutando"

.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Matar","id-ejecucion":"e-0002"}'
# → {"estado":"ok"}

.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Matar","id-ejecucion":"e-0002"}'
# → {"estado":"error","mensaje":"proceso no encontrado o ya terminado"}
```

#### Linux

```bash
send '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/usr/bin/sleep","args":["60"]}'
send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0002"}'
send '{"servicio":"ejecutor","operacion":"Estado","id-ejecucion":"e-0002"}'
send '{"servicio":"ejecutor","operacion":"Matar","id-ejecucion":"e-0002"}'
send '{"servicio":"ejecutor","operacion":"Matar","id-ejecucion":"e-0002"}'
```

### 7.4. Escenario D — Suspender / Reasumir procesos (Linux real)

> Solo en Linux es **suspensión real** vía SIGSTOP/SIGCONT. En Windows
> cambia el estado lógico pero el proceso sigue corriendo.

```bash
cat > /tmp/spam.sh <<'EOF'
#!/bin/bash
while true; do echo "tick $(date +%s)"; sleep 1; done
EOF
chmod +x /tmp/spam.sh

send '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/tmp/spam.sh"}'
send '{"servicio":"gesfich","operacion":"Crear"}'                           # f-0003
send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0003","stdout":"f-0003"}'
sleep 3

send '{"servicio":"ejecutor","operacion":"Suspender"}'
ps -eo pid,stat,cmd | grep spam.sh   # STAT empieza con T (stopped)

wc -l aralmac/ficheros/f-0003
sleep 3
wc -l aralmac/ficheros/f-0003        # mismo número (proceso parado)

send '{"servicio":"ejecutor","operacion":"Reasumir"}'
sleep 3
wc -l aralmac/ficheros/f-0003        # creció

send '{"servicio":"ejecutor","operacion":"Matar","id-ejecucion":"e-0003"}'
```

### 7.5. Escenario E — Variables de entorno

```bash
cat > /tmp/printenv.sh <<'EOF'
#!/bin/bash
echo "FOO=$FOO"
echo "BAR=$BAR"
EOF
chmod +x /tmp/printenv.sh

send '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/tmp/printenv.sh","env":["FOO=hola","BAR=mundo"]}'
send '{"servicio":"gesfich","operacion":"Crear"}'   # f-0004
send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0004","stdout":"f-0004"}'
sleep 1
send '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0004"}'
# → {"estado":"ok","contenido":"FOO=hola\nBAR=mundo\n"}
```

### 7.6. Escenario F — Máquinas de estado

#### `gesprog`: `Leer` permitido en Suspendido (figura 4)

```bash
send '{"servicio":"gesprog","operacion":"Suspender"}'           # ok
send '{"servicio":"gesprog","operacion":"Leer"}'                # ok (lista)
send '{"servicio":"gesprog","operacion":"Borrar","id-programa":"p-0001"}'
# → {"estado":"error","mensaje":"servicio suspendido"}
send '{"servicio":"gesprog","operacion":"Suspender"}'
# → {"estado":"error","mensaje":"transicion invalida"}
send '{"servicio":"gesprog","operacion":"Reasumir"}'            # ok
```

(En Windows: misma cadena de comandos con `.\scripts\send.ps1 -Mensaje '...'`)

#### `gesfich`: cualquier dato falla en Suspendido

```bash
send '{"servicio":"gesfich","operacion":"Suspender"}'
send '{"servicio":"gesfich","operacion":"Leer"}'
# → {"estado":"error","mensaje":"servicio suspendido"}
send '{"servicio":"gesfich","operacion":"Reasumir"}'
```

#### `ejecutor`: estado `Parar` rechaza nuevos

```bash
send '{"servicio":"ejecutor","operacion":"Parar"}'
send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0001"}'
# → {"estado":"error","mensaje":"servicio parando"}
```

### 7.7. Escenario G — Errores propios del `ctrllt` (PDF §3.12.3)

```bash
send '{"servicio":"foobar","operacion":"x"}'
# → {"estado":"error","mensaje":"servicio desconocido"}

send '{"servicio":"ctrllt","operacion":"Reiniciar"}'
# → {"estado":"error","mensaje":"operacion ctrllt desconocida"}

send 'esto no es JSON'
# → {"estado":"error","mensaje":"operacion ctrllt desconocida"}
```

```powershell
# Windows: parar gesfich a mano y reintentar
Stop-Process -Name gesfich
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'
# → {"estado":"error","mensaje":"servicio no conectado"}
```

```bash
# Linux: parar gesfich a mano y reintentar
pkill -f target/debug/gesfich
send '{"servicio":"gesfich","operacion":"Crear"}'
# → {"estado":"error","mensaje":"servicio no conectado"}
```

### 7.8. Escenario H — Concurrencia (varios clientes a la vez)

#### Windows

```powershell
1..20 | ForEach-Object -Parallel {
    & "$using:PWD\scripts\send.ps1" -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'
} -ThrottleLimit 10

.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Leer"}'
# → 20 IDs únicos: f-0001 .. f-0020
```

#### Linux

```bash
for i in $(seq 1 20); do send '{"servicio":"gesfich","operacion":"Crear"}' & done
wait
send '{"servicio":"gesfich","operacion":"Leer"}'
```

### 7.9. Escenario I — Shutdown global con proceso vivo

> Demuestra la flecha `Parar /Proceso == 0` de la figura 5 del PDF.

```bash
# Lanza un proceso que tarda 8 s
send '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/usr/bin/sleep","args":["8"]}'
send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0005"}'

# Apaga el sistema
send '{"servicio":"ctrllt","operacion":"Terminar"}'
# → {"estado":"ok"}

# ctrllt, gesfich, gesprog ya están muertos. ejecutor sigue vivo:
ps -ef | grep -E 'ctrllt|gesfich|gesprog|ejecutor' | grep -v grep

# Espera al sleep
sleep 9
ps -ef | grep -E 'ctrllt|gesfich|gesprog|ejecutor' | grep -v grep
# → vacío
```

### 7.10. Escenario J — Tamaño máximo de mensaje (PDF §3.8.4)

```bash
big=$(python3 -c 'print("a"*5000)')
echo "{\"servicio\":\"gesfich\",\"operacion\":\"x\",\"pad\":\"$big\"}" \
  | socat -t1 - ABSTRACT-CONNECT:pipe_ctrllt
# → conexión cerrada (mensaje > 4096 B rechazado)
```

---

## 8. Equivalencias Windows ↔ Linux

| Concepto | Windows / PowerShell | Linux / bash |
|---|---|---|
| Arrancar todo | `.\scripts\start.ps1` | `./scripts/start.sh` |
| Lanzar en background | `Start-Process ... -WindowStyle Hidden` | `... &` |
| Pausar | `Start-Sleep -Milliseconds 600` | `sleep 0.5` |
| Tubería al sistema | `\\.\pipe\pipe_ctrllt` | `@pipe_ctrllt` (abstracto) |
| Cliente JSON | `.\scripts\send.ps1 -Mensaje '...'` | `send '...'` (función) o `socat`/`ncat -U` |
| Variable opcional | `[string]$X = "default"` | `X="${X:-default}"` |
| Listar procesos | `Get-Process gesfich,gesprog,ejecutor,ctrllt` | `ps -ef \| grep -E 'ctrllt\|gesfich\|gesprog\|ejecutor'` |
| Matar todo | `Stop-Process -Name ctrllt,gesfich,gesprog,ejecutor -Force` | `pkill -f 'target/debug/(ctrllt\|gesfich\|gesprog\|ejecutor)'` |
| Fin de línea en cadenas | `\r\n` | `\n` |

---

## 9. Solución de problemas

| Síntoma | Causa probable | Solución |
|---|---|---|
| `servicio no conectado` al enviar a ctrllt | El servicio destino aún no arrancó o murió | Re-ejecuta `start.ps1` / `start.sh`; verifica con `Get-Process` / `ps -ef` |
| `Connect: timeout` en `send.ps1` | `ctrllt.exe` no está vivo | Lanza `start.ps1` o revisa si el pipe colisiona con otro proceso |
| `ABSTRACT-CONNECT` no reconocido por socat | socat antiguo | `sudo apt install --reinstall socat` o usa `ncat -U /tmp/...sock` |
| `cargo build` falla con `nix` en Windows | Targets no equivalen | El crate `nix` está en `[target.'cfg(unix)'.dependencies]`; si te falla revisa `Cargo.toml` |
| `send.ps1` imprime vacío | El servicio cerró sin enviar respuesta (mensaje > 4096 B u otro error) | Reduce el tamaño y verifica el JSON con `ConvertFrom-Json` |
| Procesos zombi en Linux | El reaper aún no ha pasado | Espera 200 ms o llama a `Estado` para forzar `actualizar_terminados` |
| Suspender no detiene el proceso en Windows | Limitación documentada (no hay `NtSuspendProcess` en stdlib) | Usar Linux para esa demo |

---

## 10. Resumen mínimo (chuleta)

```text
# Compilar
cargo build

# Arrancar (Win)
.\scripts\start.ps1
# Arrancar (Linux)
./scripts/start.sh

# Función auxiliar Linux:
send() { echo "$1" | socat -t2 - "ABSTRACT-CONNECT:${2:-pipe_ctrllt}"; }

# Crear fichero, registrar programa, ejecutar
send '{"servicio":"gesfich","operacion":"Crear"}'
send '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"/usr/bin/sort"}'
send '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0001","stdin":"f-0001","stdout":"f-0002"}'
send '{"servicio":"ejecutor","operacion":"Estado"}'
send '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0002"}'

# Apagar todo
send '{"servicio":"ctrllt","operacion":"Terminar"}'
```

