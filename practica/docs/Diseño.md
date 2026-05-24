# Diseño del Sistema: Ejecutor de Lotes

**Curso:** ST0257 - Sistemas Operativos  
**Directorio:** `docs/Diseño.md`  
**Integrantes:** Salomé Serna y Felipe Ochoa

---

## 1. Descripción general

El sistema simula un ejecutor de procesos de lotes estilo mainframe. Los
componentes son procesos independientes que se comunican exclusivamente
mediante tuberías nombradas usando mensajes en formato JSON.

El flujo básico es:

```
cliente → ctrllt → [gesprog | gesfich | ejecutor] → aralmac

```

El cliente registra programas y ficheros, luego ordena su ejecución. El
sistema los corre en segundo plano (batch) leyendo de ficheros de entrada
y escribiendo en ficheros de salida.


Ahora teniendo en cuenta la fases de entrega de la práctica, el flujo sería el siguiente 

```

Entrega 2:
cliente → [gesprog | gesfich] → aralmac

Entrega 3:
cliente → ctrllt → [gesprog | gesfich | ejecutor] → aralmac

```

---

## 2. Componentes

| Componente | Rol | Implementado por |
|---|---|---|
| `cliente` | Envía peticiones al sistema | Entregado en entrega 2 |
| `ctrllt` | Pasarela central - enruta peticiones | Equipo |
| `gesprog` | CRUD de programas en aralmac | Equipo |
| `gesfich` | CRUD de ficheros en aralmac | Equipo |
| `ejecutor` | Ejecuta procesos de lotes | Equipo |
| `aralmac` | Área de almacenamiento (directorio) | Sistema de archivos |

> **Nota de implementación:** todos los componentes están implementados en Rust.
> Los binarios se producen a través del workspace de Cargo definido en `Cargo.toml`.

---

## 3. Comunicación entre procesos

Durante la segunda entrega, el cliente puede comunicarse directamente con
`gesfich` y `gesprog`, sin pasar por `ctrllt`.

Por esta razón, el protocolo de mensajes se diseñó para soportar ambos
modos de operación:

- comunicación directa cliente → servicio
- comunicación indirecta cliente → ctrllt → servicio

### Mecanismo: tuberías nombradas (full-duplex, crate `interprocess`)

Todos los procesos se comunican a través de tuberías nombradas gestionadas
por el crate `interprocess`, que proporciona comunicación **full-duplex**
(bidireccional) en una sola tubería:

- **Windows:** Named Pipes (`\\.\pipe\<nombre>`)
- **Linux / macOS:** Unix Domain Sockets

Al ser full-duplex, basta con **una sola tubería por par de procesos**: el
servidor lee la petición y escribe la respuesta por la misma conexión. No
se necesitan dos tuberías separadas como ocurriría con FIFOs half-duplex.

```
Proceso A ══[tubería full-duplex]══ Proceso B
            petición  ──────────▶
            ◀──────────  respuesta
```

### Tuberías del sistema

Cada par de procesos tiene su propia tubería con nombre único:

| Par de procesos | Tubería |
|---|---|
| cliente ↔ ctrllt | `pipe_ctrllt` |
| ctrllt ↔ gesfich | `pipe_gesfich` |
| ctrllt ↔ gesprog | `pipe_gesprog` |
| ctrllt ↔ ejecutor | `pipe_ejecutor` |

En Windows el nombre completo es `\\.\pipe\<nombre>`; en Linux/macOS es
un socket de dominio Unix en `/tmp/<nombre>` (o la ruta que indique `-f`,
`-p`, `-e`, `-c`).

### Quién crea cada tubería

- `ctrllt` crea su propia tubería al iniciar (`pipe_ctrllt`).
- Cada servicio (`gesfich`, `gesprog`, `ejecutor`) crea su propia
  tubería al iniciar.
- El cliente se conecta a la tubería existente; no necesita crear una
  tubería de respuesta propia (la conexión full-duplex lo cubre).

---

## 4. Por qué JSON

El formato elegido para todos los mensajes es JSON por las siguientes razones:



1. **Independencia de lenguaje** - cada componente puede implementarse en
   cualquier lenguaje (Rust, Python, etc.) siempre que respete el protocolo.
   JSON es un estándar universal.

2. **Legibilidad durante el desarrollo** - los mensajes que pasan por las
   tuberías son texto plano legible. Se puede inspeccionar el tráfico
   directamente o imprimir el mensaje en consola, lo que simplifica
   enormemente la depuración.

3. **Extensibilidad** - agregar nuevos campos a un mensaje no rompe
   componentes existentes que ignoren campos desconocidos. Esto facilita
   agregar funcionalidad sin modificar todo el sistema.

4. **Disponibilidad de parsers en Rust** - el crate `serde_json` permite
   mapear cada mensaje JSON a enums tipados (`OpGesfich`, `OpGesprog`,
   `OpEjecutor`) con deserialización automática en tiempo de compilación.
   El compilador obliga a manejar todas las variantes, evitando ramas
   olvidadas.

5. **Separación entre protocolo y transporte** - el contenido JSON es
   independiente del mecanismo de transporte (Named Pipe / Unix Socket).
   Si en el futuro se quisiera cambiar el transporte, el protocolo JSON
   no cambiaría.

---

## 5. Protocolo de mensajes

Los mensajes mostrados a continuación representan el protocolo de
comunicación implementado. Todos los mensajes son JSON de una sola línea
terminada en `\n`. Tamaño máximo por mensaje: 4096 B.

### Estructura base de una petición

Todo mensaje enviado al sistema tiene la siguiente estructura:

```json
{
  "servicio":  "string",
  "operacion": "string",
  ...campos específicos de la operación
}
```

| Campo | Tipo | Descripción |
|---|---|---|
| `servicio` | string | Servicio destino: `"gesfich"`, `"gesprog"`, `"ejecutor"`, `"ctrllt"` |
| `operacion` | string | Operación solicitada al servicio (PascalCase) |
| *campos extra* | varios | Parámetros específicos de la operación, directamente en el objeto raíz |


### Estructura base de una respuesta de éxito

```json
{
  "estado": "ok",
  ...campos de resultado (opcionales según la operación)
}
```

| Campo | Tipo | Descripción |
|---|---|---|
| `estado` | string | Siempre `"ok"` cuando la operación fue exitosa |
| *campos extra* | varios | Datos devueltos por la operación, directamente en el objeto raíz |

### Estructura base de una respuesta de error

```json
{
  "estado":  "error",
  "mensaje": "string"
}
```

| Campo | Tipo | Descripción |
|---|---|---|
| `estado` | string | Siempre `"error"` cuando la operación falló |
| `mensaje` | string | Descripción del error |

### Respuesta de error (aplica a cualquier operación)

```json
{
  "estado":  "error",
  "mensaje": "Fichero f-9999 no encontrado"
}
```

---

## 6. Lógica de enrutamiento en ctrllt

`ctrllt` no ejecuta operaciones de datos directamente. Su único trabajo es:

1. Leer el mensaje JSON del cliente.
2. Inspeccionar el campo `"servicio"`.
3. Reenviar la petición al servicio correspondiente.
4. Esperar la respuesta del servicio.
5. Reenviar la respuesta al cliente.

```
Tabla de enrutamiento:

  "servicio": "gesfich"  →  pipe_gesfich
  "servicio": "gesprog"  →  pipe_gesprog
  "servicio": "ejecutor" →  pipe_ejecutor
```

### Máquina de estados de ctrllt

```
  [inicio] ──▶ [Corriendo] ──▶ [Terminar] ──▶ [Terminado]
```

`ctrllt` solo tiene dos estados operacionales: corriendo (procesando
peticiones) o terminado. No se suspende.

---

## 7. Diseño de la API por servicio


### gesfich - Gestión de ficheros

`gesfich` gestiona los ficheros que serán entrada o salida de los procesos
de lotes. Los ficheros se almacenan en `aralmac`. Cada fichero creado
recibe un identificador único con el formato `f-XXXX`.

`gesfich` puede recibir peticiones directamente desde el cliente o desde
`ctrllt`. En ambos casos, responde por la misma conexión full-duplex.

#### Máquina de estados

```
  [inicio] ──▶ [Corriendo] ──Suspender──▶ [Suspendido]
                    │                           │
                    │◀──────────Reasumir────────┘
                    │
                 Terminar
                    │
                    ▼
              [Terminado]
```

En estado `Suspendido`, `gesfich` no procesa peticiones de datos (Crear,
Leer, Actualizar, Borrar). Solo acepta `Reasumir` o `Terminar`.

#### Operaciones

---

**`Crear`** - Crea un fichero vacío en aralmac. Devuelve su identificador.

```json
// Petición
{"servicio": "gesfich", "operacion": "Crear"}

// Respuesta OK
{"estado": "ok", "id-fichero": "f-0001"}
```

---

**`Leer`** - Dos formatos:
- Con `id-fichero`: devuelve el contenido de ese fichero.
- Sin `id-fichero`: lista todos los ficheros registrados.

```json
// Petición - leer un fichero específico
{"servicio": "gesfich", "operacion": "Leer", "id-fichero": "f-0001"}

// Respuesta OK
{"estado": "ok", "id-fichero": "f-0001", "contenido": "hola mundo\n"}
```

```json
// Petición - listar todos los ficheros
{"servicio": "gesfich", "operacion": "Leer"}

// Respuesta OK
{"estado": "ok", "ficheros": [{"id-fichero": "f-0001"}, {"id-fichero": "f-0002"}]}
```

---

**`Actualizar`** - Copia el contenido de un fichero externo al fichero
identificado por `id-fichero` dentro de aralmac.

```json
// Petición
{"servicio": "gesfich", "operacion": "Actualizar", "id-fichero": "f-0001", "ruta": "/tmp/datos_entrada.txt"}

// Respuesta OK
{"estado": "ok"}
```

---

**`Borrar`** - Elimina el fichero del aralmac.

```json
// Petición
{"servicio": "gesfich", "operacion": "Borrar", "id-fichero": "f-0001"}

// Respuesta OK
{"estado": "ok"}
```

---

**`Suspender` / `Reasumir` / `Terminar`** - Operaciones de control del
servicio. No tienen campos extra.

```json
// Petición (igual estructura para las tres)
{"servicio": "gesfich", "operacion": "Suspender"}

// Respuesta OK
{"estado": "ok"}
```

---

### gesprog - Gestión de programas

`gesprog` almacena los ejecutables (binarios o scripts) con sus argumentos
y variables de ambiente en `aralmac`. Cada programa registrado recibe un
identificador único con el formato `p-XXXX`.

`gesprog` puede recibir peticiones directamente desde el cliente o desde
`ctrllt`. En ambos casos, responde por la misma conexión full-duplex.

#### Máquina de estados

```
  [Inicio] ──▶ [Corriendo] ──Suspender──▶ [Suspendido]
                    │                           │
                    │◀──────────Reasumir────────┘
                    │
                  Leer  (permitido también en Suspendido)
                    │
                 Terminar
                    │
                    ▼
              [Terminado]
```

> Nota: según el enunciado, `Leer` está permitido incluso en estado
> `Suspendido` (ver diagrama de estados del PDF).

#### Operaciones

---

**`Guardar`** - Registra un ejecutable en aralmac con sus argumentos y
ambiente. Devuelve el identificador del programa.

```json
// Petición
{"servicio": "gesprog", "operacion": "Guardar", "ejecutable": "/bin/sort", "args": ["-r", "-n"], "env": ["LANG=es_CO.UTF-8", "PATH=/usr/bin:/bin"]}

// Respuesta OK
{"estado": "ok", "id-programa": "p-0001"}
```

---

**`Leer`** - Dos formatos: con `id-programa` devuelve ese programa; sin él,
lista todos los programas registrados.

```json
// Petición - leer un programa específico
{"servicio": "gesprog", "operacion": "Leer", "id-programa": "p-0001"}

// Respuesta OK
{"estado": "ok", "id-programa": "p-0001", "ejecutable": "/bin/sort", "args": ["-r", "-n"], "env": ["LANG=es_CO.UTF-8", "PATH=/usr/bin:/bin"]}
```

```json
// Petición - listar todos los programas
{"servicio": "gesprog", "operacion": "Leer"}

// Respuesta OK
{"estado": "ok", "programas": [{"id-programa": "p-0001", "ejecutable": "/bin/sort"}, {"id-programa": "p-0002", "ejecutable": "/usr/bin/wc"}]}
```

---

**`Actualizar`** - Reemplaza los datos de un programa registrado.

```json
// Petición
{"servicio": "gesprog", "operacion": "Actualizar", "id-programa": "p-0001", "ejecutable": "/bin/sort", "args": ["-n"], "env": ["LANG=es_CO.UTF-8"]}

// Respuesta OK
{"estado": "ok"}
```

---

**`Borrar`** - Elimina el programa del aralmac.

```json
// Petición
{"servicio": "gesprog", "operacion": "Borrar", "id-programa": "p-0001"}

// Respuesta OK
{"estado": "ok"}
```

---

**`Suspender` / `Reasumir` / `Terminar`** - Igual que en `gesfich`.

```json
{"servicio": "gesprog", "operacion": "Terminar"}
```

---

### ejecutor - Ejecución de procesos de lotes

`ejecutor` toma un programa registrado en `gesprog` y ficheros registrados
en `gesfich`, y los ejecuta como un proceso de lotes real: hace `fork` +
`exec` del ejecutable, redirigiendo su `stdin` al fichero de entrada y su
`stdout` al fichero de salida. Cada proceso de lotes recibe un identificador
con el formato `e-XXXX`.

#### Máquina de estados del ejecutor (el servicio)

```
  [inicio] ──▶ [Ejecutar] ──Suspender──▶ [Suspendidos]
                    │                           │
                    │◀──────────Reasumir────────┘
                    │
                  Parar  (cuando no hay procesos activos)
                    │
                    ▼
                 [Parar]
```

#### Estados posibles de un proceso de lotes individual

| Estado | Descripción |
|---|---|
| `"Corriendo"` | El proceso está en ejecución activa |
| `"Suspendido"` | El proceso fue suspendido (SIGSTOP) |
| `"Terminado"` | El proceso finalizó normalmente |
| `"Muerto"` | El proceso fue terminado forzosamente (SIGKILL) |

#### Operaciones

---

**`Ejecutar`** - Lanza un proceso de lotes. Recibe el programa a ejecutar
y los ficheros de entrada y salida.

```json
// Petición
{"servicio": "ejecutor", "operacion": "Ejecutar", "id-programa": "p-0001", "stdin": "f-0001", "stdout": "f-0002"}

// Respuesta OK (el proceso fue lanzado)
{"estado": "ok", "id-ejecucion": "e-0001"}

// Respuesta error (programa o fichero no existe)
{"estado": "error", "mensaje": "Programa p-9999 no encontrado en aralmac"}
```

---

**`Estado`** - Dos formatos: con `id-ejecucion` devuelve el estado de ese
proceso; sin él, lista el estado de todos los procesos.

```json
// Petición - estado de un proceso específico
{"servicio": "ejecutor", "operacion": "Estado", "id-ejecucion": "e-0001"}

// Respuesta OK
{"estado": "ok", "id-ejecucion": "e-0001", "id-programa": "p-0001", "proceso-estado": "Corriendo", "codigo-salida": null}
```

```json
// Petición - listar todos los procesos
{"servicio": "ejecutor", "operacion": "Estado"}

// Respuesta OK
{"estado": "ok", "lotes": [{"id-ejecucion": "e-0001", "proceso-estado": "Corriendo"}, {"id-ejecucion": "e-0002", "proceso-estado": "Terminado"}]}
```

---

**`Matar`** - Termina forzosamente un proceso de lotes (equivale a SIGKILL).

```json
// Petición
{"servicio": "ejecutor", "operacion": "Matar", "id-ejecucion": "e-0001"}

// Respuesta OK
{"estado": "ok"}
```

---

**`Suspender` / `Reasumir` / `Parar`** - Control del servicio ejecutor.

```json
// Petición
{"servicio": "ejecutor", "operacion": "Parar"}

// Respuesta OK
{"estado": "ok"}
```

---

## 8. Flujo completo de ejemplo

El siguiente ejemplo muestra el ciclo de vida completo de un proceso de
lotes: registrar un programa, registrar los ficheros, cargar datos de
entrada, ejecutar, consultar estado y leer el resultado.

### Paso 1 - Registrar el programa `/bin/sort`

```
Cliente → ctrllt → gesprog
```

```json
// Cliente envía a ctrllt:
{"servicio": "gesprog", "operacion": "Guardar", "ejecutable": "/bin/sort", "args": ["-n"], "env": []}

// gesprog responde a ctrllt → ctrllt reenvía al cliente:
{"estado": "ok", "id-programa": "p-0001"}
```

### Paso 2 - Registrar fichero de entrada

```
Cliente → ctrllt → gesfich
```

```json
{"servicio": "gesfich", "operacion": "Crear"}
// Respuesta: {"estado": "ok", "id-fichero": "f-0001"}
```

### Paso 3 - Cargar datos en el fichero de entrada

```json
{"servicio": "gesfich", "operacion": "Actualizar", "id-fichero": "f-0001", "ruta": "/home/salo/numeros.txt"}
// Respuesta: {"estado": "ok"}
```

### Paso 4 - Registrar fichero de salida (vacío)

```json
{"servicio": "gesfich", "operacion": "Crear"}
// Respuesta: {"estado": "ok", "id-fichero": "f-0002"}
```

### Paso 5 - Ejecutar el proceso de lotes

```
Cliente → ctrllt → ejecutor
```

```json
{"servicio": "ejecutor", "operacion": "Ejecutar", "id-programa": "p-0001", "stdin": "f-0001", "stdout": "f-0002"}
// Respuesta: {"estado": "ok", "id-ejecucion": "e-0001"}
```

### Paso 6 - Consultar el estado del proceso

```json
{"servicio": "ejecutor", "operacion": "Estado", "id-ejecucion": "e-0001"}
// Respuesta: {"estado": "ok", "id-ejecucion": "e-0001", "proceso-estado": "Terminado", "codigo-salida": 0}
```

### Paso 7 - Leer el resultado

```json
{"servicio": "gesfich", "operacion": "Leer", "id-fichero": "f-0002"}
// Respuesta: {"estado": "ok", "id-fichero": "f-0002", "contenido": "1\n2\n3\n5\n8\n"}
```

---

## 9. Estructura de directorios del repositorio

```
practica/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── protocolo.rs
│   ├── ipc.rs
│   ├── estado.rs
│   ├── aralmac.rs
│   ├── ids.rs
│   └── bin/
│       ├── ctrllt.rs
│       ├── gesfich.rs
│       ├── gesprog.rs
│       └── ejecutor.rs
├── docs/
│   └── Diseño.md
└── scripts/
    ├── start.ps1
    ├── start.sh
    └── send.ps1
```

---

## 10. Resumen de identificadores

| Tipo | Formato | Ejemplo | Generado por |
|---|---|---|---|
| Fichero | `f-XXXX` | `f-0001` | `gesfich` |
| Programa | `p-XXXX` | `p-0001` | `gesprog` |
| Proceso de lotes | `e-XXXX` | `e-0001` | `ejecutor` |
