//! Tipos de petición y respuesta JSON definidos por la práctica.
//!
//! Las peticiones siguen la sección 3.8.1 del PDF:
//!     {"servicio":"<svc>","operacion":"<op>",...}
//! Las respuestas siguen 3.8.2:
//!     éxito → {"estado":"ok",...}
//!     error → {"estado":"error","mensaje":"..."}

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Envoltura mínima para detectar el destinatario antes de hacer parse tipado.
#[derive(Debug, Deserialize)]
pub struct PeticionRaiz {
    pub servicio: String,
    pub operacion: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// gesfich (sección 3.9)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")]
pub enum OpGesfich {
    Crear,
    Leer {
        #[serde(rename = "id-fichero", default)]
        id_fichero: Option<String>,
    },
    Actualizar {
        #[serde(rename = "id-fichero")]
        id_fichero: String,
        ruta: String,
    },
    Borrar {
        #[serde(rename = "id-fichero")]
        id_fichero: String,
    },
    Suspender,
    Reasumir,
    Terminar,
}

// ---------------------------------------------------------------------------
// gesprog (sección 3.10)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")]
pub enum OpGesprog {
    Guardar {
        ejecutable: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Vec<String>,
    },
    Leer {
        #[serde(rename = "id-programa", default)]
        id_programa: Option<String>,
    },
    Actualizar {
        #[serde(rename = "id-programa")]
        id_programa: String,
        ruta: String,
    },
    Borrar {
        #[serde(rename = "id-programa")]
        id_programa: String,
    },
    Suspender,
    Reasumir,
    Terminar,
}

/// Objeto de metadatos almacenado por gesprog (sección 3.10.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadatosPrograma {
    #[serde(rename = "id-programa")]
    pub id_programa: String,
    pub nombre: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    /// Ruta absoluta del binario almacenado dentro de aralmac.
    /// No forma parte del JSON expuesto al cliente.
    #[serde(skip)]
    pub ruta_aralmac: String,
}

// ---------------------------------------------------------------------------
// ejecutor (sección 3.11)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")]
pub enum OpEjecutor {
    Ejecutar {
        #[serde(rename = "id-programa")]
        id_programa: String,
        #[serde(default)]
        stdin: Option<String>,
        #[serde(default)]
        stdout: Option<String>,
        #[serde(default)]
        stderr: Option<String>,
    },
    Estado {
        #[serde(rename = "id-ejecucion", default)]
        id_ejecucion: Option<String>,
    },
    Matar {
        #[serde(rename = "id-ejecucion")]
        id_ejecucion: String,
    },
    Suspender,
    Reasumir,
    Parar,
}

// ---------------------------------------------------------------------------
// ctrllt (sección 3.12)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")]
pub enum OpCtrllt {
    Terminar,
}

// ---------------------------------------------------------------------------
// Helpers de respuesta
// ---------------------------------------------------------------------------

/// Construye una respuesta de éxito genérica: `{"estado":"ok"}`.
pub fn ok() -> String {
    json!({"estado": "ok"}).to_string()
}

/// Construye una respuesta de éxito con campos extra.
pub fn ok_con(campos: &[(&str, Value)]) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("estado".into(), Value::String("ok".into()));
    for (k, v) in campos {
        obj.insert((*k).into(), v.clone());
    }
    Value::Object(obj).to_string()
}

/// Construye una respuesta de error: `{"estado":"error","mensaje":"..."}`.
pub fn error(msg: &str) -> String {
    json!({"estado": "error", "mensaje": msg}).to_string()
}
