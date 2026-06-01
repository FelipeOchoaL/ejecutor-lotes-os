# Ejecutor de Lotes — Práctica V (Sistemas Operativos, ST0257)

Implementación en **Rust** del sistema de ejecución por lotes descrito en
`docs/ST0257-C2661-4677-Practica-V-II (1).pdf`. Está compuesto por cuatro
binarios independientes que se comunican vía **tuberías nombradas** y un
**protocolo JSON** terminado en `\n`.

> **Integrantes:** Felipe Ochoa y Salomé Serna  
> **Curso:** ST0257 — Sistemas Operativos  
> **Entrega:** Segunda + Tercera (versión II)

---

## 1. Arquitectura

```
            ┌──────────┐
 cliente ──▶│  ctrllt  │──┬──▶ gesfich  ─┐
            └──────────┘  ├──▶ gesprog  ─┼──▶ aralmac (FS)
                          └──▶ ejecutor ─┘
```

| Binario   | Responsabilidad                                              |
|-----------|--------------------------------------------------------------|
| `ctrllt`  | Pasarela: enruta peticiones al servicio del campo `servicio` |
| `gesfich` | CRUD de ficheros (`f-XXXX`) en aralmac                       |
| `gesprog` | CRUD de programas (`p-XXXX`) en aralmac                      |
| `ejecutor`| Lanza procesos de lotes (`e-XXXX`) con I/O redirigido        |

Todos los servicios comparten un directorio aralmac (parámetro `-x`) donde
se persisten ficheros y metadatos de programas.

---

## 2. ¿Por qué Rust?

| Razón | Detalle |
|-------|---------|
| **Seguridad de memoria sin GC** | El sistema lanza procesos hijos y mueve descriptores entre threads. Rust garantiza que no haya *use-after-free* ni *data races* en tiempo de compilación, algo crítico cuando se manipula `Child`, `File` y pipes simultáneamente. |
| **Concurrencia sin miedo** | Cada servicio acepta múltiples clientes a la vez con `std::thread::spawn`. `Arc<Mutex<>>` deja claro qué estado se comparte (estado del servicio, mapa de procesos del ejecutor) y el compilador rechaza accesos no sincronizados. |
| **Tipado fuerte para protocolos** | `serde` + `serde_json` convierten cada mensaje JSON del enunciado en `enum` tipados (`OpGesfich`, `OpGesprog`, `OpEjecutor`). El compilador obliga a manejar todas las variantes, evitando ramas olvidadas. |
| **Cross-platform real** | Una sola base de código produce binarios nativos para Linux y Windows 11 (requisito para grupos de dos), gracias a `interprocess` para tuberías nombradas y `std::process::Command` para lanzar procesos. |
| **Cero costo en runtime** | Los abstractions de Rust se compilan a código nativo equivalente a C, sin overhead. El ejecutor puede manejar muchos procesos sin penalización. |
| **Toolchain unificada** | `cargo build` produce los cuatro binarios. No hace falta `Makefile`, ni linkers manuales, ni gestionar dependencias por separado. |
| **Errores como valores** | El protocolo del PDF distingue éxito/error con campos JSON. `Result<T, &'static str>` mapea naturalmente a esa semántica. |

---

## 3. Estructura del repositorio

```
practica/
├── Cargo.toml
├── src/
│   ├── lib.rs              # librería compartida (módulos abajo)
│   ├── protocolo.rs        # tipos serde del protocolo (sección 3.7+ del PDF)
│   ├── ipc.rs              # tuberías nombradas cross-platform (interprocess)
│   ├── estado.rs           # máquinas de estados (figuras 3, 4 y 5 del PDF)
│   ├── aralmac.rs          # utilidades de almacenamiento
│   ├── ids.rs              # generación de identificadores f-/p-/e-XXXX
│   └── bin/
│       ├── ctrllt.rs       # pasarela
│       ├── gesfich.rs      # gestor de ficheros
│       ├── gesprog.rs      # gestor de programas
│       └── ejecutor.rs     # ejecutor de lotes
├── docs/
│   ├── ST0257-C2661-4677-Practica-V-II (1).pdf
│   ├── explicacion.md      # explicación detallada del código
│   └── ejecucion.md        # guía de comandos y escenarios de prueba
└── scripts/
    ├── start.ps1           # arranque Windows
    ├── start.sh            # arranque Linux/macOS
    └── send.ps1            # envío de mensajes de prueba (Windows)
```

---

## 4. Requisitos

- **Rust** 1.70 o superior (`rustup` recomendado).
- **Windows 10/11** o **Linux/macOS** (testeado con `cargo 1.95.0`).
- Sin dependencias del sistema: las tuberías nombradas se gestionan a
  través del crate `interprocess`.

---

## 5. Compilación

```bash
cargo build               # debug, todos los binarios
cargo build --release     # release, optimizado
```

Tras compilar, los binarios estarán en `target/debug/` (o `target/release/`).

---

## 6. Ejecución

### 6.1. Sinopsis (la del PDF)

```
ctrllt   -c <pipe_cliente>  -f <pipe_gesfich>  -p <pipe_gesprog>  -e <pipe_ejecutor>
gesfich  -f <pipe_gesfich>  -x <dir_aralmac>
gesprog  -p <pipe_gesprog>  -x <dir_aralmac>
ejecutor -e <pipe_ejecutor> -x <dir_aralmac>
```

Las opciones `-a`, `-b`, `--resp-gesprog`, `-d` (tuberías de respuesta
de half-duplex) se aceptan pero no se usan: la IPC subyacente es
**full-duplex** (Named Pipes en Windows, Unix Domain Sockets en Linux),
por lo que basta con la tubería de petición. Es uno de los dos modos
contemplados por el PDF (sección 3.1).

> **Nota:** el PDF reutiliza `-c` para gesprog. Para evitar el conflicto
> de clap, el flag largo de respuesta de gesprog es `--resp-gesprog`.

### 6.2. Arranque rápido en Windows (PowerShell)

```powershell
cd practica
.\scripts\start.ps1
```

Eso compila si hace falta, crea `aralmac/`, y lanza los cuatro servicios
en segundo plano. El cliente debe conectarse a `\\.\pipe\pipe_ctrllt`.

### 6.3. Arranque rápido en Linux/macOS

```bash
cd practica
chmod +x scripts/start.sh
./scripts/start.sh
```

### 6.4. Arranque manual (Windows / PowerShell)

```powershell
mkdir aralmac
Start-Process .\target\debug\gesfich.exe  -ArgumentList '-f','pipe_gesfich','-x','aralmac' -WindowStyle Hidden
Start-Process .\target\debug\gesprog.exe  -ArgumentList '-p','pipe_gesprog','-x','aralmac' -WindowStyle Hidden
Start-Process .\target\debug\ejecutor.exe -ArgumentList '-e','pipe_ejecutor','-x','aralmac' -WindowStyle Hidden
Start-Sleep -Milliseconds 500
Start-Process .\target\debug\ctrllt.exe   -ArgumentList '-c','pipe_ctrllt','-f','pipe_gesfich','-p','pipe_gesprog','-e','pipe_ejecutor' -WindowStyle Hidden
```

### 6.5. Arranque manual (Linux / bash)

```bash
mkdir -p /tmp/aralmac
./target/debug/gesfich  -f gesfich_req  -x /tmp/aralmac &
./target/debug/gesprog  -p gesprog_req  -x /tmp/aralmac &
./target/debug/ejecutor -e ejecutor_req -x /tmp/aralmac &
sleep 0.3
./target/debug/ctrllt   -c ctrllt_req -f gesfich_req -p gesprog_req -e ejecutor_req &
```

---

## 7. Envío de mensajes

El protocolo es JSON-de-una-línea sobre la tubería. Cada conexión envía
una petición y recibe una respuesta. Tamaño máximo por mensaje: 4096 B.

### 7.1. Desde PowerShell (sin cliente compilado)

```powershell
.\scripts\send.ps1 -Pipe pipe_ctrllt -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'
# → {"estado":"ok","id-fichero":"f-0001"}
```

### 7.2. Desde Linux (con `socat`)

```bash
# Una sola vez por sesión: define la función helper
send() { echo "$1" | socat -t2 - "ABSTRACT-CONNECT:${2:-pipe_ctrllt}"; }

# Uso
send '{"servicio":"gesfich","operacion":"Crear"}'
# → {"estado":"ok","id-fichero":"f-0001"}

# Bypass del ctrllt (hablar directo con un servicio)
send '{"servicio":"gesfich","operacion":"Crear"}' pipe_gesfich
```

> En Linux las "tuberías nombradas" del PDF se implementan con Unix Domain
> Sockets en el namespace abstracto (`@pipe_ctrllt`). De ahí
> `ABSTRACT-CONNECT` en `socat`.

### 7.3. Cualquier lenguaje

Sólo hace falta abrir el pipe/socket, escribir la línea JSON y leer la
respuesta. Cualquier cliente que sepa hablar con Named Pipes (Windows)
o Unix Domain Sockets (Linux) sirve.

---

## 8. Ejemplo completo: ordenar números con `sort.exe`

Probado en Windows. En Linux se sustituye `C:\Windows\System32\sort.exe`
por `/usr/bin/sort`.

```powershell
# 1) Preparar fichero local con datos
"5`n2`n8`n1`n3" | Out-File -Encoding ascii -NoNewline aralmac\input.txt

# 2) Crear ficheros de entrada y salida en aralmac
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'                                     # f-0001
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'                                     # f-0002

# 3) Cargar input.txt dentro de f-0001
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Actualizar","id-fichero":"f-0001","ruta":"aralmac\\input.txt"}'

# 4) Registrar sort.exe como programa
.\scripts\send.ps1 -Mensaje '{"servicio":"gesprog","operacion":"Guardar","ejecutable":"C:\\Windows\\System32\\sort.exe","args":[],"env":[]}'

# 5) Ejecutar: stdin=f-0001 (numeros), stdout=f-0002 (resultado)
.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Ejecutar","id-programa":"p-0001","stdin":"f-0001","stdout":"f-0002"}'

# 6) Consultar estado del proceso
.\scripts\send.ps1 -Mensaje '{"servicio":"ejecutor","operacion":"Estado","id-ejecucion":"e-0001"}'
# → {"estado":"ok","id-ejecucion":"e-0001","id-programa":"p-0001","proceso-estado":"Terminado","codigo-salida":0}

# 7) Leer el resultado
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Leer","id-fichero":"f-0002"}'
# → {"estado":"ok","contenido":"1\r\n2\r\n3\r\n5\r\n8\r\n"}

# 8) Apagar todo el sistema
.\scripts\send.ps1 -Mensaje '{"servicio":"ctrllt","operacion":"Terminar"}'
```

---

## 9. Resumen del protocolo (PDF sección 3)

### 9.1. Petición

```json
{"servicio": "<svc>", "operacion": "<op>", ...campos extra}
```

### 9.2. Respuesta de éxito

```json
{"estado": "ok", ...campos extra}
```

### 9.3. Respuesta de error

```json
{"estado": "error", "mensaje": "<descripcion>"}
```

### 9.4. Operaciones por servicio

| Servicio  | Operaciones                                                          |
|-----------|----------------------------------------------------------------------|
| `gesfich` | `Crear`, `Leer`, `Actualizar`, `Borrar`, `Suspender`, `Reasumir`, `Terminar` |
| `gesprog` | `Guardar`, `Leer`, `Actualizar`, `Borrar`, `Suspender`, `Reasumir`, `Terminar` |
| `ejecutor`| `Ejecutar`, `Estado`, `Matar`, `Suspender`, `Reasumir`, `Parar`      |
| `ctrllt`  | `Terminar`                                                           |

La explicación detallada con ejemplos JSON para cada operación está en
`docs/explicacion.md`.

---

## 10. Limitaciones conocidas

- **Suspender/Reasumir de procesos hijos en Windows.** Linux usa
  `SIGSTOP/SIGCONT` vía `nix`. Windows no expone una API estable para
  suspender procesos arbitrarios sin `NtSuspendProcess` (kernel32 no la
  documenta), así que en Windows la suspensión cambia sólo el estado
  lógico del proceso de lotes y el binario hijo sigue corriendo. El
  servicio responde `{"estado":"ok"}` para conservar la semántica del PDF.
- **IDs no persistentes.** Los contadores `f-`, `p-`, `e-` arrancan en 1
  cada vez que se lanza el servicio. El PDF no exige persistencia.
- **Una petición por conexión.** Cada conexión TCP/pipe transporta una
  pareja petición/respuesta y se cierra. El cliente debe re-conectar
  para cada operación. El PDF no impone lo contrario.

---

## 11. Pruebas rápidas

### Windows

```powershell
cargo build
.\scripts\start.ps1
.\scripts\send.ps1 -Mensaje '{"servicio":"gesfich","operacion":"Crear"}'
.\scripts\send.ps1 -Mensaje '{"servicio":"ctrllt","operacion":"Terminar"}'
```

### Linux

```bash
cargo build
./scripts/start.sh
send() { echo "$1" | socat -t2 - "ABSTRACT-CONNECT:${2:-pipe_ctrllt}"; }
send '{"servicio":"gesfich","operacion":"Crear"}'
send '{"servicio":"ctrllt","operacion":"Terminar"}'
```

> Para una **guía completa** con escenarios de prueba (CRUD, ejecución de
> procesos, suspender/reasumir, concurrencia, shutdown ordenado), consulta
> [`docs/ejecucion.md`](practica/docs/ejecucion.md).

---

## 12. Referencias

- PDF de la práctica: `docs/ST0257-C2661-4677-Practica-V-II (1).pdf`
- Explicación detallada del código: [`docs/explicacion.md`](practica/docs/explicacion.md)
- Guía de ejecución y escenarios de prueba: [`docs/ejecucion.md`](practica/docs/ejecucion.md)
