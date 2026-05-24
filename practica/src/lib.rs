//! Librería compartida entre los cuatro binarios del sistema ctrllt.
//!
//! Cumple el protocolo descrito en `docs/ST0257-C2661-4677-Practica-V-II (1).pdf`,
//! sección 3.7 y siguientes ("Formato de mensajes JSON").
//!
//! - `protocolo`: tipos serde para peticiones y respuestas JSON.
//! - `ipc`: tuberías nombradas cross-platform (named pipes en Windows,
//!   Unix domain sockets en Linux/macOS).
//! - `estado`: máquinas de estados de los servicios (Corriendo/Suspendido/Terminado/Parar).
//! - `aralmac`: utilidades de almacenamiento sobre el directorio aralmac.
//! - `ids`: generación de identificadores f-XXXX, p-XXXX, e-XXXX.

#![forbid(unsafe_code)]

pub mod protocolo;
pub mod ipc;
pub mod estado;
pub mod aralmac;
pub mod ids;

/// Longitud máxima de un mensaje JSON (sección 3.8.4 del PDF).
pub const MSG_MAX_LEN: usize = 4096;
