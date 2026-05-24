# Explicación detallada del código

Este documento describe, módulo por módulo y función por función, cómo
está implementado el sistema y a qué parte del PDF
`ST0257-C2661-4677-Practica-V-II (1).pdf` corresponde cada pieza.

> Las referencias al PDF tienen la forma **(PDF §3.X)**.

---

## 0. Mapa mental rápido

```
src/
├── lib.rs            # Declara los módulos y la constante MSG_MAX_LEN.
├── protocolo.rs      # Tipos de petición y respuesta JSON.       (PDF §3.7–3.12)
├── ipc.rs            # Tuberías nombradas cross-platform.        (PDF §3.1)
├── estado.rs         # Máquinas de estados.                      (PDF §3.4.2, 3.5.2, 3.6.2, 3.12)
├── aralmac.rs        # Estructura del repositorio en disco.       (PDF §3.4, 3.5)
├── ids.rs            # Generación de IDs f-XXXX, p-XXXX, e-XXXX. (PDF §3.8.3)
└── bin/
    ├── ctrllt.rs     # Pasarela y único punto de entrada de clientes. (PDF §3.3, §3.12)
    ├── gesfich.rs    # Gestor de ficheros.                        (PDF §3.4, §3.9)
    ├── gesprog.rs    # Gestor de programas.                       (PDF §3.5, §3.10)
    └── ejecutor.rs   # Ejecutor de procesos de lotes.             (PDF §3.6, §3.11)
```

Flujo de un mensaje:

```
                 (1) JSON línea               (2) JSON línea
   cliente ──────────────────▶ ctrllt ──────────────────▶ servicio
   cliente ◀────────────────── ctrllt ◀────────────────── servicio
                 (4) respuesta                (3) respuesta
```

---

## 1. `src/lib.rs`

Una librería minúscula que sólo expone módulos públicos y constantes.

```rust
pub const MSG_MAX_LEN: usize = 4096;
```

Corresponde al límite de la **§3.8.4** del PDF
(`MSG_MAX_LEN = 4096 bytes por mensaje`). Se aplica en `ipc::Sesion`.

---

## 2. `src/protocolo.rs` — Mensajes JSON (PDF §3.7–3.12)

Aquí viven los tipos `serde` que mapean uno a uno con los mensajes que
define el PDF.

### 2.1. `PeticionRaiz`

```rust
pub struct PeticionRaiz {
    pub servicio: String,
    pub operacion: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}
```

Lectura mínima de la cabecera **§3.8.1**:

```json
{"servicio":"<svc>","operacion":"<op>",...}
```

`ctrllt` la usa para decidir a qué servicio reenviar la petición sin
parsear el resto. Cada servicio luego deserializa el mensaje completo a
su enumerado tipado.

### 2.2. `OpGesfich` (PDF §3.9.1)

```rust
#[derive(Deserialize)]
#[serde(tag = "operacion")]
pub enum OpGesfich {
    Crear,
    Leer { id_fichero: Option<String> },
    Actualizar { id_fichero: String, ruta: String },
    Borrar { id_fichero: String },
    Suspender, Reasumir, Terminar,
}
```

- `#[serde(tag = "operacion")]` hace que `serde` distinga la variante
  por el campo `operacion` (en lugar del nombre del enum).
- Cada variante captura los campos que el PDF exige para esa operación.
- `Leer` admite `id-fichero` opcional: con valor → lee uno; sin
  valor → lista todos (**§3.9.1**, formato listar todos).

### 2.3. `OpGesprog` (PDF §3.10.1)

Igual estructura, con la peculiaridad de `Guardar`:

```rust
Guardar {
    ejecutable: String,
    #[serde(default)] args: Vec<String>,
    #[serde(default)] env: Vec<String>,
},
```

`args` y `env` son opcionales tal como permite el PDF
(*"Los campos args y env son opcionales."*).

### 2.4. `MetadatosPrograma` (PDF §3.10.3)

```rust
pub struct MetadatosPrograma {
    pub id_programa: String,
    pub nombre: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    #[serde(skip)]
    pub ruta_aralmac: String,
}
```

Lo que se devuelve al cliente bajo la clave `programa` cuando hace
`Leer` por id. El campo `ruta_aralmac` se conserva sólo en disco y
**no** se filtra al cliente (`#[serde(skip)]` + filtrado explícito).

### 2.5. `OpEjecutor` (PDF §3.11.1)

```rust
Ejecutar {
    id_programa: String,
    stdin: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
},
Estado   { id_ejecucion: Option<String> },
Matar    { id_ejecucion: String },
Suspender, Reasumir, Parar,
```

`stdin`/`stdout`/`stderr` son IDs de fichero (`f-XXXX`); son opcionales
porque el PDF dice que se heredan los descriptores del servicio si se
omiten.

### 2.6. `OpCtrllt` (PDF §3.12.1)

Sólo permite `Terminar`. Cualquier otra cosa devuelve
`"operacion ctrllt desconocida"` (mensaje exacto de **§3.12.3**).

### 2.7. Constructores de respuestas

```rust
pub fn ok() -> String                          // {"estado":"ok"}
pub fn ok_con(campos: &[(&str, Value)]) -> String
pub fn error(msg: &str) -> String              // {"estado":"error","mensaje":msg}
```

Centralizan la sección **§3.8.2**. Todos los servicios construyen
respuestas con estos helpers, así que el formato es siempre uniforme.

---

## 3. `src/ipc.rs` — Tuberías nombradas (PDF §3.1)

El PDF habla de tuberías nombradas con dos sabores: full-duplex
(Windows) o half-duplex (Linux/FIFOs). Implementamos el sabor
full-duplex con el crate `interprocess`, que mapea a:

- **Windows:** Named Pipes (`\\.\pipe\<nombre>`)
- **Linux/macOS:** Unix Domain Sockets

Esto cumple la frase del PDF *"En algunos sistemas operativos las
tuberías nombradas son full-duplex, lo que necesitará una sola
tubería."* (**§3.1**).

### 3.1. `nombre_base()`

Acepta tanto `/tmp/fifo_ctrllt_req` como `pipe_ctrllt` y devuelve sólo
la parte final del path. Razones:

- En Linux, `interprocess` con `GenericNamespaced` ignora directorios.
- En Windows, los Named Pipes viven en el namespace `\\.\pipe\`. Un
  path absoluto no tiene sentido ahí.

### 3.2. `crear_listener(nombre)` y `conectar(nombre)`

Wrappers de `ListenerOptions::new().name(...).create_sync()` y
`Stream::connect(...)`. Construyen el nombre con
`GenericNamespaced` para portabilidad.

### 3.3. `Sesion`

```rust
pub struct Sesion { lector: BufReader<Stream> }
```

Encapsula una conexión cliente. La diseñé así para tener `read_line()`
con buffer (necesario porque las tuberías no garantizan que un
`read()` devuelva una línea completa) y a la vez poder escribir a
través del mismo stream con `BufReader::get_mut`.

- `leer_mensaje()` lee una línea y verifica `MSG_MAX_LEN` (PDF
  §3.8.4). Devuelve `None` en EOF.
- `escribir_mensaje()` añade `\n` y hace `flush`.

### 3.4. `solicitar(nombre, mensaje)`

Función de conveniencia que abre conexión, escribe la petición y lee
la respuesta. Se usa en `ctrllt` para reenviar peticiones a los
servicios destino.

---

## 4. `src/estado.rs` — Máquinas de estados

### 4.1. `EstadoServicio` (PDF figuras 3 y 4)

```rust
pub enum EstadoServicio { Corriendo, Suspendido, Terminado }
```

Transiciones permitidas (idénticas a las figuras):

| Origen      | Suspender   | Reasumir    | Terminar    |
|-------------|-------------|-------------|-------------|
| Corriendo   | Suspendido  | error       | Terminado   |
| Suspendido  | error       | Corriendo   | Terminado   |
| Terminado   | error       | error       | error       |

Cualquier transición inválida devuelve `"transicion invalida"`
(mensaje exacto del PDF, **§3.9.2 / §3.10.2**).

### 4.2. `EstadoEjecutor` (PDF figura 5)

```rust
pub enum EstadoEjecutor { Ejecutar, Suspendidos, Parar }
```

La diferencia con los otros servicios es:

- No tiene `Suspendido` sino `Suspendidos` (en plural: refiere a los
  procesos).
- En lugar de `Terminar` directo hay `Parar`: no acepta nuevas
  ejecuciones y termina cuando los procesos activos llegan a cero
  (`/Proceso == 0` en la figura).

`acepta_nuevos()` se usa en `Ejecutar` para devolver
`"servicio parando"` si el estado es `Parar`.

---

## 5. `src/aralmac.rs` — Repositorio en disco (PDF §3.4, §3.5)

El PDF dice que `<info-aralmac>` puede ser "la ruta de un directorio".
Esta implementación usa esa interpretación.

Distribución física:

```
<aralmac>/
├── ficheros/
│   ├── f-0001         ← contenido binario del fichero
│   └── f-0002
└── programas/
    ├── p-0001.json    ← metadatos (id, nombre, args, env, ruta interna)
    └── p-0001.bin     ← copia del ejecutable original
```

Funciones públicas:

| Función                            | Qué hace                                                  |
|------------------------------------|-----------------------------------------------------------|
| `dir_ficheros(base)`               | Devuelve `<base>/ficheros/` y la crea si no existe.       |
| `dir_programas(base)`              | Análogo para programas.                                   |
| `ruta_fichero(base, id)`           | `<base>/ficheros/<id>`                                    |
| `ruta_meta_programa(base, id)`     | `<base>/programas/<id>.json`                              |
| `ruta_bin_programa(base, id)`      | `<base>/programas/<id>.bin`                               |
| `listar_ficheros(base)`            | Lista IDs `f-*` ordenados (para `Leer` sin id en gesfich). |
| `listar_programas(base)`           | Análogo para programas.                                    |

Cualquier fallo de I/O se traduce a `"error al listar ficheros"` /
`"error al listar programas"` (PDF §3.9.2 / §3.10.2).

---

## 6. `src/ids.rs` — Identificadores (PDF §3.8.3)

```rust
pub struct Generador { prefijo: &'static str, contador: AtomicU32 }
```

- `siguiente()` devuelve `<prefijo>-{:04}` usando un contador atómico,
  thread-safe sin necesidad de Mutex.
- `ajustar_minimo(valor)` está pensado para reanudar contadores tras
  escanear aralmac (no usado en esta entrega, pero deja la puerta abierta
  a persistencia futura).

---

## 7. `src/bin/gesfich.rs` — Gestor de ficheros (PDF §3.4 y §3.9)

### 7.1. CLI

```
gesfich -f <tuberia> [-b <tuberia>] -x <aralmac>
```

`-f` es la tubería de peticiones (la única usada en full-duplex).
`-b` se acepta pero no se usa. `-x` apunta al directorio aralmac.

### 7.2. Bucle principal

```rust
for conexion in listener.incoming() {
    thread::spawn(move || atender(svc, conexion));
}
```

Cada conexión va a su propio thread. El estado del servicio
(`EstadoServicio` + contador de IDs + ruta aralmac) está dentro de un
`Arc<Servicio>` para ser compartido sin bloqueos innecesarios.

### 7.3. `atender()`

1. Lee una línea JSON de la sesión.
2. Llama a `procesar()` para obtener `(respuesta, debe_terminar)`.
3. Escribe la respuesta.
4. Si `debe_terminar`, sale del proceso con `process::exit(0)` tras
   asegurar que el `flush` de la respuesta se completó.

### 7.4. `procesar()`

Decisiones implementadas exactamente como pide el PDF:

| Caso                                            | Resultado                                        |
|-------------------------------------------------|---------------------------------------------------|
| JSON inválido                                   | `error("operacion desconocida")`                  |
| `servicio` ≠ `"gesfich"`                        | `error("servicio desconocido")`                   |
| Operaciones de control (Suspender/Reasumir/Terminar) | Transición de la máquina o `error("transicion invalida")` |
| Operación de datos con estado Suspendido        | `error("servicio suspendido")`                    |

### 7.5. Operaciones de datos

| Operación   | Comportamiento (PDF §3.9.1) |
|-------------|------------------------------|
| `Crear`     | Genera `f-NNNN`, crea archivo vacío en `aralmac/ficheros/`, devuelve `{"estado":"ok","id-fichero":"f-NNNN"}`. |
| `Leer` con id | Lee el contenido del archivo. Si no existe → `"fichero no encontrado"`. |
| `Leer` sin id | Devuelve la lista de IDs presentes en el directorio. |
| `Actualizar` | `fs::copy(ruta_fuente, aralmac/ficheros/<id>)`. Si el destino no existe → `"fichero no encontrado"`; si la copia falla → `"no se pudo actualizar el fichero"`. |
| `Borrar`    | `fs::remove_file`. |

Cada error usa el mensaje literal del PDF, sin parafrasear.

---

## 8. `src/bin/gesprog.rs` — Gestor de programas (PDF §3.5 y §3.10)

Misma estructura que gesfich. Diferencias clave:

### 8.1. `Leer` permitido en estado Suspendido

Por la figura 4 del PDF, el `Leer` es válido incluso cuando el
servicio está suspendido. Por eso `procesar()` compara el match de
`Leer` **antes** de la guarda general
`_ if !estado.esta_corriendo() => error("servicio suspendido")`.

### 8.2. `guardar()` (PDF §3.10.1 `Guardar`)

1. Verifica que `ejecutable` apunte a un archivo existente
   (`"no se pudo guardar el programa"` si no).
2. Genera `p-NNNN` y calcula `nombre = basename(ejecutable)`.
3. Copia el binario a `aralmac/programas/p-NNNN.bin`.
4. Escribe el JSON de metadatos en `p-NNNN.json` con
   `{id-programa, nombre, args, env, ruta_aralmac}`. El campo
   `ruta_aralmac` es interno y nunca se devuelve al cliente.
5. Si cualquier paso falla, se hace rollback (borra el .bin si el .json
   no se pudo escribir) y se devuelve `"no se pudo guardar el programa"`.

### 8.3. `leer()` por id (PDF §3.10.2)

Carga el `.json`, le quita `ruta_aralmac` y lo devuelve bajo la clave
`programa`, exactamente con el shape:

```json
{"estado":"ok","programa":{"id-programa":"p-0001","nombre":"sort.exe","args":[],"env":[]}}
```

### 8.4. `actualizar()` (PDF §3.10.1)

Sobreescribe `p-NNNN.bin` con el contenido del fichero indicado por
`ruta`, y actualiza `nombre` en los metadatos al basename nuevo.

### 8.5. `borrar()`

Borra el `.bin` (best-effort) y el `.json`. Si el `.json` no existe,
devuelve `"programa no encontrado"`.

---

## 9. `src/bin/ejecutor.rs` — Ejecutor de procesos (PDF §3.6 y §3.11)

### 9.1. Estructuras

```rust
struct ProcesoInfo {
    id_ejecucion: String,
    id_programa: String,
    hijo: Option<Child>,
    estado: String,            // "Ejecutando" | "Suspendido" | "Terminado"
    codigo_salida: Option<i32>,
}

struct Servicio {
    estado: Mutex<EstadoEjecutor>,
    gen: Generador,
    aralmac: PathBuf,
    procesos: Mutex<HashMap<String, ProcesoInfo>>,
}
```

`procesos` es un mapa `id_ejecucion → ProcesoInfo`. Lo protejo con
`Mutex` porque el reaper, los handlers de petición y los kills lo
modifican concurrentemente.

### 9.2. Thread reaper

```rust
fn reaper(svc: Arc<Servicio>) {
    loop {
        thread::sleep(Duration::from_millis(200));
        actualizar_terminados(&mut svc.procesos.lock().unwrap());
        ...
        if estado_global == EstadoEjecutor::Parar && activos == 0 {
            process::exit(0);
        }
    }
}
```

Cumple dos funciones:

1. **Cosecha** procesos terminados llamando `try_wait()` sobre cada
   `Child`, actualizando estado y `codigo-salida`.
2. **Detecta condición de parada** (PDF figura 5: la transición
   `Parar` con `/Proceso == 0`). Cuando el servicio está en `Parar`
   y no quedan procesos activos, sale del proceso.

### 9.3. `ejecutar()` (PDF §3.11.1)

Paso a paso:

1. Comprueba estado del servicio: si está en `Suspendidos` →
   `"servicio suspendido"`; si está en `Parar` → `"servicio parando"`.
2. Carga `aralmac/programas/<id-programa>.json`; si no existe →
   `"no se pudo ejecutar el programa"`.
3. Construye un `std::process::Command` con:
   - el ejecutable copiado en aralmac (`ruta_aralmac`),
   - los args del JSON,
   - las variables de entorno del JSON (`"K=V"` → `cmd.env(k, v)`),
   - `stdin` abierto desde `aralmac/ficheros/<stdin>` (o `Stdio::null()`),
   - `stdout`/`stderr` truncados y escritos en aralmac/ficheros/...
4. `spawn()`. Si falla → `"no se pudo ejecutar el programa"`.
5. Genera `e-NNNN`, guarda `ProcesoInfo` y devuelve el id.

### 9.4. `estado()` (PDF §3.11.2)

Actualiza primero los procesos terminados (`actualizar_terminados`).

- Con `id-ejecucion`: si no existe → `"proceso no encontrado"`. Si
  existe → JSON con `id-ejecucion`, `id-programa`, `proceso-estado` y
  (si terminado) `codigo-salida`.
- Sin id: devuelve la lista de todos los procesos.

### 9.5. `matar()` (PDF §3.11.1)

`Child::kill()` + `Child::wait()`. Marca estado `"Terminado"`. Si el
proceso ya estaba `Terminado`, devuelve
`"proceso no encontrado o ya terminado"`.

### 9.6. `Suspender` / `Reasumir`

Cambia el estado global del servicio y luego itera sobre los
procesos para enviarles señales.

**Unix:** SIGSTOP / SIGCONT vía `nix::sys::signal::kill`.

**Windows:** Sin API estable. La función es un *no-op* documentado en
el README; el estado lógico cambia pero el proceso hijo sigue
ejecutándose. Las pruebas del corrector deberían probar la suspensión
en Linux según el PDF (sección 4 requisito 2 del propio PDF).

### 9.7. `Parar`

Transición de estado a `Parar`. El reaper se ocupa de hacer
`process::exit(0)` cuando todos los hijos hayan terminado. Esto
implementa la flecha `Parar /Proceso = 0` de la figura 5.

---

## 10. `src/bin/ctrllt.rs` — Pasarela (PDF §3.3 y §3.12)

### 10.1. CLI

```
ctrllt -c <pipe_cliente>
       -f <pipe_gesfich>
       -p <pipe_gesprog>
       -e <pipe_ejecutor>
       [--resp-ctrllt N] [--resp-gesfich N] [--resp-gesprog N] [--resp-ejecutor N]
```

Las tuberías de respuesta (`-a` / `-b` / `-c` / `-d` en el PDF) están
como flags largos para evitar el conflicto del PDF, donde `-c` se
asigna dos veces.

### 10.2. `enrutar()`

```rust
match raiz.servicio.as_str() {
    "ctrllt"   => match raiz.operacion.as_str() {
        "Terminar" => (terminar_sistema(rutas), true),
        _          => (error("operacion ctrllt desconocida"), false),
    },
    "gesfich"  => (reenviar(&rutas.gesfich,  mensaje, "gesfich"),  false),
    "gesprog"  => (reenviar(&rutas.gesprog,  mensaje, "gesprog"),  false),
    "ejecutor" => (reenviar(&rutas.ejecutor, mensaje, "ejecutor"), false),
    _          => (error("servicio desconocido"), false),
}
```

Refleja literalmente la sección **§3.12.3** del PDF (mensajes de
error idénticos).

### 10.3. `reenviar()`

1. Hace un `conectar()` de prueba para verificar que el servicio
   destino acepta conexiones; si falla → `"servicio no conectado"`
   (mensaje del PDF).
2. Llama a `solicitar()` para enviar la petición tal cual y leer la
   respuesta. Si la comunicación falla durante el reenvío →
   `"error enviando solicitud al servicio"` o
   `"error leyendo respuesta del servicio"`.
3. La respuesta del servicio se devuelve **sin modificar** al cliente.

### 10.4. `terminar_sistema()` (PDF §3.12.1)

```rust
solicitar(gesfich,  r#"{"servicio":"gesfich","operacion":"Terminar"}"#);
solicitar(gesprog,  r#"{"servicio":"gesprog","operacion":"Terminar"}"#);
solicitar(ejecutor, r#"{"servicio":"ejecutor","operacion":"Parar"}"#);
ok()
```

- gesfich y gesprog reciben `Terminar` → se apagan limpiamente.
- ejecutor recibe `Parar` → no acepta nuevos, espera a sus hijos y
  termina cuando llegan a cero.
- ctrllt responde `{"estado":"ok"}` y sale con `process::exit(0)`
  desde el handler.

---

## 11. Mapeo PDF → código

| Sección PDF                          | Archivo                          | Símbolo / función                |
|--------------------------------------|----------------------------------|----------------------------------|
| §3.1 Tipo de comunicación            | `src/ipc.rs`                     | `crear_listener`, `conectar`     |
| §3.3 ctrllt                          | `src/bin/ctrllt.rs`              | `enrutar`, `reenviar`            |
| §3.4 gesfich                         | `src/bin/gesfich.rs`             | `procesar`, `crear`, `leer`, ... |
| §3.5 gesprog                         | `src/bin/gesprog.rs`             | idem                             |
| §3.6 ejecutor                        | `src/bin/ejecutor.rs`            | `ejecutar`, `estado`, `matar`    |
| §3.7 Formato JSON                    | `src/protocolo.rs`               | `ok`, `ok_con`, `error`          |
| §3.8.3 Identificadores               | `src/ids.rs`                     | `Generador::siguiente`           |
| §3.8.4 MSG_MAX_LEN                   | `src/lib.rs`, `ipc.rs`           | `MSG_MAX_LEN`, `Sesion::leer_mensaje` |
| §3.9 gesfich (operaciones)           | `src/bin/gesfich.rs`             | match en `procesar`              |
| §3.10 gesprog (operaciones)          | `src/bin/gesprog.rs`             | match en `procesar`              |
| §3.11 ejecutor (operaciones)         | `src/bin/ejecutor.rs`            | match en `procesar`              |
| §3.12 ctrllt (operaciones)           | `src/bin/ctrllt.rs`              | `enrutar`, `terminar_sistema`    |
| Figura 3 (gesfich estados)           | `src/estado.rs`                  | `EstadoServicio`                 |
| Figura 4 (gesprog estados)           | `src/estado.rs` + ramas Leer     | `EstadoServicio`                 |
| Figura 5 (ejecutor estados)          | `src/estado.rs` + reaper         | `EstadoEjecutor`, `reaper`       |

---

## 12. Garantías y elecciones de diseño

- **Una petición ↔ una respuesta por conexión.** Simplifica el manejo
  de errores y de límites de mensaje. El PDF no exige multiplexar.
- **Concurrencia por thread por conexión.** Soporta múltiples clientes
  simultáneos como pide la sección 3.2 ("El sistema soporta múltiples
  cliente ejecutándose simultáneamente"). El estado compartido se
  protege con `Mutex`.
- **Procesos del ejecutor desligados del IO del servicio.** Las
  redirecciones a `f-XXXX` se hacen con `Stdio::from(File)`, lo que
  cierra los descriptores en el padre y los pasa al hijo. Esto evita
  que el padre acabe leyendo o escribiendo accidentalmente sobre los
  ficheros del proceso de lotes.
- **Errores con mensajes literales del PDF.** Todos los mensajes de
  error (`"fichero no encontrado"`, `"servicio suspendido"`, etc.) son
  copias textuales del enunciado para facilitar la corrección
  automatizada.
- **Sin `unsafe`.** El crate ejecuta con `#![forbid(unsafe_code)]` en
  `lib.rs`, garantizando que ningún módulo introduzca código inseguro.
