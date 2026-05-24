//! Tuberías nombradas cross-platform.
//!
//! El PDF (sección 3.1) admite dos modelos de tuberías nombradas:
//!   - full-duplex (Windows Named Pipes) → una sola tubería por enlace.
//!   - half-duplex (FIFOs de Linux)      → dos tuberías por enlace.
//!
//! Esta implementación usa `interprocess::local_socket`, que en Windows
//! abre Named Pipes (`\\.\pipe\<nombre>`) y en Unix abre Unix Domain Sockets
//! (`/tmp/<nombre>.sock` aprox.). Ambos son full-duplex: una sola conexión
//! transporta la petición y su respuesta. Los argumentos `-a/-b/-c/-d` del
//! enunciado (tuberías opcionales de respuesta) se aceptan pero no se usan.
//!
//! Los mensajes son una línea JSON terminada en '\n' con longitud máxima
//! `MSG_MAX_LEN = 4096` bytes (sección 3.8.4).

use crate::MSG_MAX_LEN;
use interprocess::local_socket::{
    prelude::*, GenericNamespaced, ListenerOptions, Stream,
};

/// Re-exporta el trait `ListenerExt` para que los binarios puedan llamar
/// a `listener.incoming()` sin importarlo manualmente.
pub use interprocess::local_socket::traits::ListenerExt;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

/// Devuelve el nombre base (sin directorios) de la tubería.
///
/// Acepta rutas tipo `/tmp/fifo_ctrllt_req` o nombres sueltos
/// `fifo_ctrllt_req` y los normaliza para usar como nombre de
/// namespace en Windows / socket en Linux.
fn nombre_base(nombre: &str) -> String {
    Path::new(nombre)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| nombre.to_string())
}

/// Crea un *listener* (servidor) que escucha conexiones en la tubería
/// indicada. Sustituye al `mkfifo` del enunciado.
pub fn crear_listener(
    nombre: &str,
) -> io::Result<interprocess::local_socket::Listener> {
    let n = nombre_base(nombre);
    let name = n.as_str().to_ns_name::<GenericNamespaced>()?;
    ListenerOptions::new().name(name).create_sync()
}

/// Conecta como cliente a una tubería existente.
pub fn conectar(nombre: &str) -> io::Result<Stream> {
    let n = nombre_base(nombre);
    let name = n.as_str().to_ns_name::<GenericNamespaced>()?;
    Stream::connect(name)
}

/// Sesión de mensajería JSON sobre un stream local.
///
/// Cada sesión corresponde a una conexión entrante: el cliente envía
/// una petición JSON y la sesión escribe la respuesta JSON.
pub struct Sesion {
    lector: BufReader<Stream>,
}

impl Sesion {
    pub fn nueva(stream: Stream) -> Self {
        Self {
            lector: BufReader::new(stream),
        }
    }

    /// Lee una línea JSON. Devuelve `None` si el peer cerró sin enviar nada.
    pub fn leer_mensaje(&mut self) -> io::Result<Option<String>> {
        let mut linea = String::new();
        let bytes = self.lector.read_line(&mut linea)?;
        if bytes == 0 {
            return Ok(None);
        }
        if linea.len() > MSG_MAX_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mensaje excede MSG_MAX_LEN",
            ));
        }
        Ok(Some(
            linea.trim_end_matches(['\r', '\n']).to_string(),
        ))
    }

    /// Escribe un mensaje JSON terminado en '\n'.
    pub fn escribir_mensaje(&mut self, msg: &str) -> io::Result<()> {
        let s = self.lector.get_mut();
        s.write_all(msg.as_bytes())?;
        s.write_all(b"\n")?;
        s.flush()?;
        Ok(())
    }
}

/// Conveniencia: abre conexión, envía petición y lee una respuesta.
///
/// La usa `ctrllt` para reenviar peticiones a los servicios destino.
pub fn solicitar(nombre_tuberia: &str, mensaje: &str) -> io::Result<String> {
    let stream = conectar(nombre_tuberia)?;
    let mut sesion = Sesion::nueva(stream);
    sesion.escribir_mensaje(mensaje)?;
    match sesion.leer_mensaje()? {
        Some(r) => Ok(r),
        None => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "el servicio cerró sin responder",
        )),
    }
}
