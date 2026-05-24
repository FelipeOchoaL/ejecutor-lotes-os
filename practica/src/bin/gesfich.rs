//! Servicio gesfich — Gestión de ficheros en aralmac.
//!
//! Implementa la sección 3.9 del PDF de la práctica:
//! operaciones Crear / Leer / Actualizar / Borrar / Suspender / Reasumir / Terminar.

use clap::Parser;
use practica::aralmac;
use practica::estado::EstadoServicio;
use practica::ids::Generador;
use practica::ipc::{crear_listener, ListenerExt, Sesion};
use practica::protocolo::{error, ok, ok_con, OpGesfich, PeticionRaiz};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{fs, process, thread};

/// gesfich -f <tuberia-nombrada> [-b <tuberia-nombrada>] -x <info-aralmac>
#[derive(Parser, Debug)]
#[command(name = "gesfich", about = "Gestor de ficheros del sistema ctrllt")]
struct Cli {
    /// Tubería de peticiones (también de respuestas si la IPC es full-duplex).
    #[arg(short = 'f')]
    f: String,

    /// Tubería de respuestas (sólo half-duplex). No usada con local sockets.
    #[arg(short = 'b')]
    b: Option<String>,

    /// Ruta del directorio aralmac.
    #[arg(short = 'x')]
    x: PathBuf,
}

struct Servicio {
    estado: Mutex<EstadoServicio>,
    gen: Generador,
    aralmac: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = fs::create_dir_all(&cli.x) {
        eprintln!("gesfich: no se pudo crear aralmac {}: {}", cli.x.display(), e);
        process::exit(1);
    }

    let listener = match crear_listener(&cli.f) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("gesfich: no se pudo abrir la tuberia {}: {}", cli.f, e);
            process::exit(1);
        }
    };

    let svc = Arc::new(Servicio {
        estado: Mutex::new(EstadoServicio::Corriendo),
        gen: Generador::nuevo("f"),
        aralmac: cli.x.clone(),
    });

    eprintln!("gesfich: escuchando en {} (aralmac={})", cli.f, cli.x.display());

    for conexion in listener.incoming() {
        let conexion = match conexion {
            Ok(c) => c,
            Err(e) => {
                eprintln!("gesfich: error aceptando conexión: {}", e);
                continue;
            }
        };
        let svc = Arc::clone(&svc);
        thread::spawn(move || atender(svc, conexion));
    }
}

fn atender(svc: Arc<Servicio>, stream: interprocess::local_socket::Stream) {
    let mut sesion = Sesion::nueva(stream);
    let mensaje = match sesion.leer_mensaje() {
        Ok(Some(m)) => m,
        _ => return,
    };

    let (respuesta, terminar) = procesar(&svc, &mensaje);
    let _ = sesion.escribir_mensaje(&respuesta);

    if terminar {
        eprintln!("gesfich: Terminar recibido, saliendo");
        process::exit(0);
    }
}

/// Devuelve (respuesta_json, debe_terminar_proceso).
fn procesar(svc: &Servicio, mensaje: &str) -> (String, bool) {
    let raiz: PeticionRaiz = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(_) => return (error("operacion desconocida"), false),
    };
    if raiz.servicio != "gesfich" {
        return (error("servicio desconocido"), false);
    }

    let op: OpGesfich = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(_) => return (error("operacion desconocida"), false),
    };

    let estado_actual = *svc.estado.lock().unwrap();

    match op {
        OpGesfich::Suspender => match estado_actual.suspender() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                (ok(), false)
            }
            Err(m) => (error(m), false),
        },
        OpGesfich::Reasumir => match estado_actual.reasumir() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                (ok(), false)
            }
            Err(m) => (error(m), false),
        },
        OpGesfich::Terminar => match estado_actual.terminar() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                (ok(), true)
            }
            Err(m) => (error(m), false),
        },
        // ---- Operaciones de datos: bloqueadas si está Suspendido ----
        _ if !estado_actual.esta_corriendo() => (error("servicio suspendido"), false),

        OpGesfich::Crear => (crear(svc), false),
        OpGesfich::Leer { id_fichero } => (leer(svc, id_fichero), false),
        OpGesfich::Actualizar { id_fichero, ruta } => {
            (actualizar(svc, &id_fichero, &ruta), false)
        }
        OpGesfich::Borrar { id_fichero } => (borrar(svc, &id_fichero), false),
    }
}

fn crear(svc: &Servicio) -> String {
    let id = svc.gen.siguiente();
    let ruta = aralmac::ruta_fichero(&svc.aralmac, &id);
    if let Some(p) = ruta.parent() {
        let _ = fs::create_dir_all(p);
    }
    match fs::File::create(&ruta) {
        Ok(_) => ok_con(&[("id-fichero", Value::String(id))]),
        Err(_) => error("no se pudo crear el fichero"),
    }
}

fn leer(svc: &Servicio, id: Option<String>) -> String {
    match id {
        Some(id) => {
            let ruta = aralmac::ruta_fichero(&svc.aralmac, &id);
            if !ruta.exists() {
                return error("fichero no encontrado");
            }
            match fs::read_to_string(&ruta) {
                Ok(c) => ok_con(&[("contenido", Value::String(c))]),
                Err(_) => error("fichero no encontrado"),
            }
        }
        None => match aralmac::listar_ficheros(&svc.aralmac) {
            Ok(lista) => ok_con(&[("ficheros", json!(lista))]),
            Err(_) => error("error al listar ficheros"),
        },
    }
}

fn actualizar(svc: &Servicio, id: &str, ruta_fuente: &str) -> String {
    let destino = aralmac::ruta_fichero(&svc.aralmac, id);
    if !destino.exists() {
        return error("fichero no encontrado");
    }
    match fs::copy(ruta_fuente, &destino) {
        Ok(_) => ok(),
        Err(_) => error("no se pudo actualizar el fichero"),
    }
}

fn borrar(svc: &Servicio, id: &str) -> String {
    let ruta = aralmac::ruta_fichero(&svc.aralmac, id);
    if !ruta.exists() {
        return error("fichero no encontrado");
    }
    match fs::remove_file(&ruta) {
        Ok(_) => ok(),
        Err(_) => error("fichero no encontrado"),
    }
}
