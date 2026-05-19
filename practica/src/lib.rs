//! Código compartido entre servicios: protocolo JSON, tipos comunes, utilidades de FIFOs, etc.
//!
//! Cada proceso es un binario en `src/bin/`; este crate solo expone lo que quieras reutilizar.


#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

// constante
pub const MSG_MAX_LEN: usize = 4096; //4096 bytes maximo de caracteres en mensaje

//contadoe  funciona aunque haya varios hilos lehyendo o escribiendo
static CONTADOR: AtomicU32 = AtomicU32::new(1);

//genera un id unico para el fichero con el prefijo
pub fn siguiente_id(prefijo: &str) -> String {
    let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
    format!("{}-{:04}", prefijo, n)
}

//maquina de estados de gesfich
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoServicio {
    Corriendo,
    Suspendido,
    Terminado,
}

impl EstadoServicio {
    // Retorna Ok con el nuevo estado, o Err con mensaje de error
    pub fn suspender(&self) -> Result<EstadoServicio, String> {
        match self {
            EstadoServicio::Corriendo => Ok(EstadoServicio::Suspendido),
            _ => Err("transicion invalida".to_string()),
        }
    }

    pub fn reanudar(&self) -> Result<EstadoServicio, String> {
        match self {
            EstadoServicio::Suspendido => Ok(EstadoServicio::Corriendo),
            _ => Err("transicion invalida".to_string()),
        }
    }

    pub fn esta_disponible(&self) -> bool {
        matches!(self, EstadoServicio::Corriendo)
    }
}



//==
// peticion Gesfich
//==
#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")] 
pub enum PeticionGesfich {
    Crear,
    Leer {
        #[serde(rename = "id-fichero")]
        id_fichero: Option<String>,
    }
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

//==
// peticion Gesprog
//==
#[derive(debug, Deserialize)]
#[serde(tag = "operacion")]
pub enum PeticionGesprog{
    Guardar {
        ejecutablede: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Vec<String>,
    },
    Leer {
        #[serde(rename = "id-programa")]
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

//==
// peticion Ejecutoe
//==
#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")]
pub enum PeticionEjecutor {
    Ejecutar {
        #[serde(rename = "id-programa")]
        id_programa: String,
        // Los tres el enunciado
        #[serde(rename = "stdin")]
        stdin: Option<String>,
        #[serde(rename = "stdout")]
        stdout: Option<String>,
        #[serde(rename = "stderr")]
        stderr: Option<String>,
    },

    Estado {
        #[serde(rename = "id-ejecucion")]
        id_ejecucion: Option<String>,
    },

    Matar {
        #[serde(rename = "id-ejecucion")]
        id_ejecucion: String,
    },
    Suspender,
    Reasumir,
    Parar
}

//==
//JSON Respuesta
//==

#[derive(Debug, Serialize)]
pub struct RespuestaOk {
    pub estado: &'static str,
    #[serde(rename = "id-fichero", skip_serializinf_if = "Option::is_none")]
    pub id_fichero: Option<String>,
    #[serde(rename = "id-programa", skip_serializinf_if = "Option::is_none")]
    pub id_programa: Option<String>,
    #[serde(rename = "id-ejecucion", skip_serializinf_if = "Option::is_none")]
    pub id_ejecucion: Option<String>,

    #[serde(skip_serializinf_if = "Option::is_none")]
    pub contenido: Option<String>,
    #[serde(skip_serializinf_if = "Option::is_none")]
    pub ficheros: Option<Vec<String>>,
    #[serde(skip_serializinf_if = "Option::is_none")]
    pub programas: Option<MetadatosPrograma>,
    #[serde(rename = "proceso-estado", skip_serializing_if = "Option::is_none")]
    pub proceso_estado: Option<String>,
    #[serde(rename = "codigo-salida", skip_serializing_if = "Option::is_none")]
    pub codigo_salida: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operacion")] // con este el serde sabe que tipo de operacion es
pub enum Respuesta {
    #[serde(rename = "ok")]
    Ok {
        #[serde(rename = "id-fichero", skip_serializing_if = "Option::is_none")]
        id_fichero: Option<String>,
        #[serde(skip_serializinf_if = "Option::is_none")]
        contenido: Option<String>
        #[serde(skip_serializing_if = "Option::is_none")]
        ficheros: Option<Vec<String>>,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

use std::io::{BufReader, BufWriter, BufRead, Read, Write, path};
use std::fs::OpenOptions;

//leemos una linea de la tuberia
pub fn leer_mensaje(path: &str) -> std::io::result<String> {
    let archivo = OpenOptions::new().read(true).open(path)?;
    let mut lector = BufReader::new(archivo);
    let mut linea = String::new();
    lector.read_line(&mut linea)?;
    Ok(linea.trim_end().to_string())
}

//escribimos un mensaje json terminado en \n
pub fn escribir_mensaje(path: &str, mensaje: &str) -> std::io::Result<()> {
    let mut archivo = OpenOptions::new().write(true).open(path)?;
    writeln!(archivo, "{}", mensaje)?;
    Ok(())
}

// //leemos un mensaje json terminado en \n
// pub fn leer_mensaje_json(path: &str) -> std::io::Result<PeticionGesfich> {
//     let mensaje = leer_mensaje(path)?;
//     let peticion: PeticionGesfich = serde_json::from_str(&mensaje)?;
//     Ok(peticion)
// }



//escribimos un mensaje json terminado en \n