# Diseño del Sistema: Ejecutor de Lotes

**Curso:** ST0257 - Sistemas Operativos  
**Directorio:** `docs/Diseño.md`  
**Integrantes:** Salomé Serna y Felipe Ochoa

---

## 1. Descripción general

El sistema simula un ejecutor de procesos de lotes estilo mainframe. Los
componentes son procesos independientes que se comunican exclusivamente
mediante tuberías nombradas (FIFOs) usando mensajes en formato JSON.

El flujo básico es:

```
cliente → ctrllt → [gesprog | gesfich | ejecutor] → aralmac
```

El cliente registra programas y ficheros, luego ordena su ejecución. El
sistema los corre en segundo plano (batch) leyendo de ficheros de entrada
y escribiendo en ficheros de salida.

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

---

## 3. Comunicación entre procesos

### Mecanismo: tuberías nombradas (FIFOs)

Todos los procesos se comunican exclusivamente a través de tuberías
nombradas. En Linux, los FIFOs son **half-duplex** (unidireccionales), por
lo tanto se requieren **dos tuberías por conexión**: una para la petición
y otra para la respuesta.

```
Proceso A ──[pipe_peticion]──▶ Proceso B
Proceso A ◀──[pipe_respuesta]── Proceso B
```

### Tuberías del sistema

Cada par de procesos tiene sus propias tuberías con nombre único:

| Par de procesos | Tubería de petición | Tubería de respuesta |
|---|---|---|
| cliente ↔ ctrllt | `/tmp/fifo_ctrllt_req` | `/tmp/fifo_ctrllt_resp_<pid>` |
| ctrllt ↔ gesfich | `/tmp/fifo_gesfich_req` | `/tmp/fifo_gesfich_resp` |
| ctrllt ↔ gesprog | `/tmp/fifo_gesprog_req` | `/tmp/fifo_gesprog_resp` |
| ctrllt ↔ ejecutor | `/tmp/fifo_ejecutor_req` | `/tmp/fifo_ejecutor_resp` |

La tubería de respuesta del cliente incluye el PID del proceso cliente(`<pid>`) para que múltiples clientes simultáneos tengan tuberías de respuesta únicas.

### Quién crea cada tubería

- `ctrllt` crea sus propias tuberías al iniciar (`fifo_ctrllt_*`).
- Cada servicio (`gesfich`, `gesprog`, `ejecutor`) crea sus propias
  tuberías al iniciar.
- El cliente crea su tubería de respuesta personal al iniciar.

---

## 4. Por qué JSON

El formato elegido para todos los mensajes es JSON por las siguientes razones:



1. **Independencia de lenguaje** - cada componente puede implementarse en
   cualquier lenguaje (C, Python, etc.) siempre que respete el protocolo.
   JSON es un estándar universal.

2. **Legibilidad durante el desarrollo** - los mensajes que pasan por los
   FIFOs son texto plano legible. Se puede inspeccionar el tráfico
   directamente con `cat` sobre el FIFO o imprimir el mensaje en consola,
   lo que simplifica enormemente la depuración.

3. **Extensibilidad** - agregar nuevos campos a un mensaje no rompe
   componentes existentes que ignoren campos desconocidos. Esto facilita
   agregar funcionalidad sin modificar todo el sistema.

4. **Disponibilidad de parsers en C** - librerías como `cJSON` o `json-c`
   permiten serializar y deserializar JSON en C sin dificultad, sin
   necesidad de implementar un parser propio.

5. **Separación entre protocolo y transporte** - el contenido JSON es
   independiente del mecanismo de transporte (FIFO). Si en el futuro se
   quisiera cambiar el transporte (por ejemplo a sockets), el protocolo
   JSON no cambiaría.

---

## 5. Protocolo de mensajes

### Estructura base de una petición

Todo mensaje enviado al sistema tiene la siguiente estructura:

```json
{
  "service":    "string",
  "action":     "string",
  "request_id": "string",
  "data":       {}
}
```

| Campo | Tipo | Descripción |
|---|---|---|
| `service` | string | Servicio destino: `"gesfich"`, `"gesprog"`, `"ejecutor"` |
| `action` | string | Operación a ejecutar (ver sección de cada servicio) |
| `request_id` | string | Identificador único de la petición (UUID). Permite correlacionar respuestas con peticiones en un entorno de múltiples clientes |
| `data` | objeto | Parámetros específicos de la operación. Puede ser `{}` si la operación no requiere parámetros |

### Estructura base de una respuesta

```json
{
  "status":     "ok | error",
  "request_id": "string",
  "message":    "string",
  "data":       {}
}
```

| Campo | Tipo | Descripción |
|---|---|---|
| `status` | string | `"ok"` si la operación fue exitosa, `"error"` si falló |
| `request_id` | string | El mismo `request_id` de la petición original |
| `message` | string | Descripción del resultado. En errores, explica qué salió mal |
| `data` | objeto | Datos devueltos por la operación. `{}` si no hay datos |

### Respuesta de error (aplica a cualquier operación)

```json
{
  "status":     "error",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "message":    "Fichero f-9999 no encontrado",
  "data":       {}
}
```

---

## 6. Lógica de enrutamiento en ctrllt

`ctrllt` no ejecuta operaciones de datos directamente. Su único trabajo es:

1. Leer el mensaje JSON del cliente.
2. Inspeccionar el campo `"service"`.
3. Reenviar el mensaje completo (sin modificarlo) a la tubería del servicio correspondiente.
4. Esperar la respuesta del servicio.
5. Reenviar la respuesta al cliente.

```
Tabla de enrutamiento:

  "service": "gesfich"  →  /tmp/fifo_gesfich_req
  "service": "gesprog"  →  /tmp/fifo_gesprog_req
  "service": "ejecutor" →  /tmp/fifo_ejecutor_req
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

**`crear`** - Crea un fichero vacío en aralmac. Devuelve su identificador.

```json
// Petición
{
  "service":    "gesfich",
  "action":     "crear",
  "request_id": "550e8400-e29b-41d4-a716-446655440001",
  "data":       {}
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440001",
  "message":    "Fichero creado exitosamente",
  "data": {
    "id_fichero": "f-0001"
  }
}
```

---

**`leer`** - Dos formatos:
- Con `id_fichero`: devuelve el contenido de ese fichero.
- Sin `id_fichero`: lista todos los ficheros registrados.

```json
// Petición - leer un fichero específico
{
  "service":    "gesfich",
  "action":     "leer",
  "request_id": "550e8400-e29b-41d4-a716-446655440002",
  "data": {
    "id_fichero": "f-0001"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440002",
  "message":    "",
  "data": {
    "id_fichero": "f-0001",
    "contenido":  "hola mundo\n"
  }
}
```

```json
// Petición - listar todos los ficheros
{
  "service":    "gesfich",
  "action":     "leer",
  "request_id": "550e8400-e29b-41d4-a716-446655440003",
  "data":       {}
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440003",
  "message":    "",
  "data": {
    "ficheros": [
      { "id_fichero": "f-0001" },
      { "id_fichero": "f-0002" }
    ]
  }
}
```

---

**`actualizar`** - Copia el contenido de un fichero externo al fichero
identificado por `id_fichero` dentro de aralmac.

```json
// Petición
{
  "service":    "gesfich",
  "action":     "actualizar",
  "request_id": "550e8400-e29b-41d4-a716-446655440004",
  "data": {
    "id_fichero":  "f-0001",
    "ruta_fuente": "/tmp/datos_entrada.txt"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440004",
  "message":    "Fichero f-0001 actualizado",
  "data":       {}
}
```

---

**`borrar`** - Elimina el fichero del aralmac.

```json
// Petición
{
  "service":    "gesfich",
  "action":     "borrar",
  "request_id": "550e8400-e29b-41d4-a716-446655440005",
  "data": {
    "id_fichero": "f-0001"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440005",
  "message":    "Fichero f-0001 eliminado",
  "data":       {}
}
```

---

**`suspender` / `reasumir` / `terminar`** - Operaciones de control del
servicio. No tienen parámetros en `data`.

```json
// Petición (igual estructura para las tres)
{
  "service":    "gesfich",
  "action":     "suspender",
  "request_id": "550e8400-e29b-41d4-a716-446655440006",
  "data":       {}
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440006",
  "message":    "Servicio gesfich suspendido",
  "data":       {}
}
```

---

### gesprog - Gestión de programas

`gesprog` almacena los ejecutables (binarios o scripts) con sus argumentos
y variables de ambiente en `aralmac`. Cada programa registrado recibe un
identificador único con el formato `p-XXXX`.

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

**`guardar`** - Registra un ejecutable en aralmac con sus argumentos y
ambiente. Devuelve el identificador del programa.

```json
// Petición
{
  "service":    "gesprog",
  "action":     "guardar",
  "request_id": "550e8400-e29b-41d4-a716-446655440010",
  "data": {
    "ejecutable": "/bin/sort",
    "argumentos": ["-r", "-n"],
    "ambiente":   ["LANG=es_CO.UTF-8", "PATH=/usr/bin:/bin"]
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440010",
  "message":    "Programa registrado exitosamente",
  "data": {
    "id_programa": "p-0001"
  }
}
```

---

**`leer`** - Dos formatos: con `id_programa` devuelve ese programa; sin él,
lista todos los programas registrados.

```json
// Petición - leer un programa específico
{
  "service":    "gesprog",
  "action":     "leer",
  "request_id": "550e8400-e29b-41d4-a716-446655440011",
  "data": {
    "id_programa": "p-0001"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440011",
  "message":    "",
  "data": {
    "id_programa": "p-0001",
    "ejecutable":  "/bin/sort",
    "argumentos":  ["-r", "-n"],
    "ambiente":    ["LANG=es_CO.UTF-8", "PATH=/usr/bin:/bin"]
  }
}
```

```json
// Petición - listar todos los programas
{
  "service":    "gesprog",
  "action":     "leer",
  "request_id": "550e8400-e29b-41d4-a716-446655440012",
  "data":       {}
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440012",
  "message":    "",
  "data": {
    "programas": [
      { "id_programa": "p-0001", "ejecutable": "/bin/sort" },
      { "id_programa": "p-0002", "ejecutable": "/usr/bin/wc" }
    ]
  }
}
```

---

**`actualizar`** - Reemplaza los datos de un programa registrado.

```json
// Petición
{
  "service":    "gesprog",
  "action":     "actualizar",
  "request_id": "550e8400-e29b-41d4-a716-446655440013",
  "data": {
    "id_programa": "p-0001",
    "ejecutable":  "/bin/sort",
    "argumentos":  ["-n"],
    "ambiente":    ["LANG=es_CO.UTF-8"]
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440013",
  "message":    "Programa p-0001 actualizado",
  "data":       {}
}
```

---

**`borrar`** - Elimina el programa del aralmac.

```json
// Petición
{
  "service":    "gesprog",
  "action":     "borrar",
  "request_id": "550e8400-e29b-41d4-a716-446655440014",
  "data": {
    "id_programa": "p-0001"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440014",
  "message":    "Programa p-0001 eliminado",
  "data":       {}
}
```

---

**`suspender` / `reasumir` / `terminar`** - Igual que en `gesfich`.

```json
{
  "service":    "gesprog",
  "action":     "terminar",
  "request_id": "550e8400-e29b-41d4-a716-446655440015",
  "data":       {}
}
```

---

### ejecutor - Ejecución de procesos de lotes

`ejecutor` toma un programa registrado en `gesprog` y ficheros registrados
en `gesfich`, y los ejecuta como un proceso de lotes real: hace `fork` +
`exec` del ejecutable, redirigiendo su `stdin` al fichero de entrada y su
`stdout` al fichero de salida. Cada proceso de lotes recibe un identificador
con el formato `l-XXXX`.

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
| `"corriendo"` | El proceso está en ejecución activa |
| `"suspendido"` | El proceso fue suspendido (SIGSTOP) |
| `"terminado"` | El proceso finalizó normalmente |
| `"muerto"` | El proceso fue terminado forzosamente (SIGKILL) |

#### Operaciones

---

**`ejecutar`** - Lanza un proceso de lotes. Recibe el programa a ejecutar
y los ficheros de entrada y salida.

```json
// Petición
{
  "service":    "ejecutor",
  "action":     "ejecutar",
  "request_id": "550e8400-e29b-41d4-a716-446655440020",
  "data": {
    "id_programa":        "p-0001",
    "id_fichero_entrada": "f-0001",
    "id_fichero_salida":  "f-0002"
  }
}

// Respuesta OK (el proceso fue lanzado)
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440020",
  "message":    "Proceso de lotes iniciado",
  "data": {
    "id_lote": "l-0001"
  }
}

// Respuesta error (programa o fichero no existe)
{
  "status":     "error",
  "request_id": "550e8400-e29b-41d4-a716-446655440020",
  "message":    "Programa p-9999 no encontrado en aralmac",
  "data":       {}
}
```

---

**`estado`** - Dos formatos: con `id_lote` devuelve el estado de ese
proceso; sin él, lista el estado de todos los procesos.

```json
// Petición - estado de un proceso específico
{
  "service":    "ejecutor",
  "action":     "estado",
  "request_id": "550e8400-e29b-41d4-a716-446655440021",
  "data": {
    "id_lote": "l-0001"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440021",
  "message":    "",
  "data": {
    "id_lote": "l-0001",
    "estado":   "corriendo"
  }
}
```

```json
// Petición - listar todos los procesos
{
  "service":    "ejecutor",
  "action":     "estado",
  "request_id": "550e8400-e29b-41d4-a716-446655440022",
  "data":       {}
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440022",
  "message":    "",
  "data": {
    "lotes": [
      { "id_lote": "l-0001", "estado": "corriendo"  },
      { "id_lote": "l-0002", "estado": "terminado"  }
    ]
  }
}
```

---

**`matar`** - Termina forzosamente un proceso de lotes (equivale a SIGKILL).

```json
// Petición
{
  "service":    "ejecutor",
  "action":     "matar",
  "request_id": "550e8400-e29b-41d4-a716-446655440023",
  "data": {
    "id_lote": "l-0001"
  }
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440023",
  "message":    "Proceso l-0001 terminado",
  "data":       {}
}
```

---

**`suspender` / `reasumir` / `parar`** - Control del servicio ejecutor.

```json
// Petición
{
  "service":    "ejecutor",
  "action":     "parar",
  "request_id": "550e8400-e29b-41d4-a716-446655440024",
  "data":       {}
}

// Respuesta OK
{
  "status":     "ok",
  "request_id": "550e8400-e29b-41d4-a716-446655440024",
  "message":    "Servicio ejecutor detenido",
  "data":       {}
}
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
{ "service": "gesprog", "action": "guardar", "request_id": "req-1",
  "data": { "ejecutable": "/bin/sort", "argumentos": ["-n"], "ambiente": [] } }

// gesprog responde a ctrllt → ctrllt reenvía al cliente:
{ "status": "ok", "request_id": "req-1", "message": "Programa registrado",
  "data": { "id_programa": "p-0001" } }
```

### Paso 2 - Registrar fichero de entrada

```
Cliente → ctrllt → gesfich
```

```json
{ "service": "gesfich", "action": "crear", "request_id": "req-2", "data": {} }
// Respuesta: { "id_fichero": "f-0001" }
```

### Paso 3 - Cargar datos en el fichero de entrada

```json
{ "service": "gesfich", "action": "actualizar", "request_id": "req-3",
  "data": { "id_fichero": "f-0001", "ruta_fuente": "/home/salo/numeros.txt" } }
```

### Paso 4 - Registrar fichero de salida (vacío)

```json
{ "service": "gesfich", "action": "crear", "request_id": "req-4", "data": {} }
// Respuesta: { "id_fichero": "f-0002" }
```

### Paso 5 - Ejecutar el proceso de lotes

```
Cliente → ctrllt → ejecutor
```

```json
{ "service": "ejecutor", "action": "ejecutar", "request_id": "req-5",
  "data": { "id_programa": "p-0001",
            "id_fichero_entrada": "f-0001",
            "id_fichero_salida": "f-0002" } }
// Respuesta: { "id_lote": "l-0001" }
```

### Paso 6 - Consultar el estado del proceso

```json
{ "service": "ejecutor", "action": "estado", "request_id": "req-6",
  "data": { "id_lote": "l-0001" } }
// Respuesta: { "data": { "id_lote": "l-0001", "estado": "terminado" } }
```

### Paso 7 - Leer el resultado

```json
{ "service": "gesfich", "action": "leer", "request_id": "req-7",
  "data": { "id_fichero": "f-0002" } }
// Respuesta: { "data": { "id_fichero": "f-0002", "contenido": "1\n2\n3\n5\n8\n" } }
```

---

## 9. Estructura de directorios del repositorio

```
/
├── docs/
│   └── Diseño.md          ← este archivo
├── ctrllt/
│   └── ctrllt.c
├── gesfich/
│   └── gesfich.c
├── gesprog/
│   └── gesprog.c
├── ejecutor/
│   └── ejecutor.c
├── common/
│   └── protocol.h         ← estructuras y constantes compartidas
└── README.md
```

---

## 10. Resumen de identificadores

| Tipo | Formato | Ejemplo | Generado por |
|---|---|---|---|
| Fichero | `f-XXXX` | `f-0001` | `gesfich` |
| Programa | `p-XXXX` | `p-0001` | `gesprog` |
| Proceso de lotes | `l-XXXX` | `l-0001` | `ejecutor` |
| Petición | UUID v4 | `550e8400-...` | `cliente` |
