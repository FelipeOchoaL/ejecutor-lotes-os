//! Servicio gesprog — Gestión de programas en aralmac.
//!
//! Implementa la sección 3.10 del PDF de la práctica.
//! Operaciones: Guardar / Leer / Actualizar / Borrar / Suspender / Reasumir / Terminar.
//!
//! La figura 4 del PDF permite `Leer` incluso en estado Suspendido.

use clap::Parser;
use practica::aralmac;
use practica::estado::EstadoServicio;
use practica::ids::Generador;
use practica::ipc::{crear_listener, ListenerExt, Sesion};
use practica::protocolo::{
    error, ok, ok_con, MetadatosPrograma, OpGesprog, PeticionRaiz,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{fs, process, thread};

/// gesprog -p <tuberia-nombrada> [-c <tuberia-nombrada>] -x <info-aralmac>
#[derive(Parser, Debug)]
#[command(name = "gesprog", about = "Gestor de programas del sistema ctrllt")]
struct Cli {
    #[arg(short = 'p')]
    p: String,
    #[arg(short = 'c')]
    c: Option<String>,
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
        eprintln!("gesprog: no se pudo crear aralmac {}: {}", cli.x.display(), e);
        process::exit(1);
    }

    let listener = match crear_listener(&cli.p) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("gesprog: no se pudo abrir la tuberia {}: {}", cli.p, e);
            process::exit(1);
        }
    };

    let svc = Arc::new(Servicio {
        estado: Mutex::new(EstadoServicio::Corriendo),
        gen: Generador::nuevo("p"),
        aralmac: cli.x.clone(),
    });

    eprintln!("gesprog: escuchando en {} (aralmac={})", cli.p, cli.x.display());

    for conexion in listener.incoming() {
        let conexion = match conexion {
            Ok(c) => c,
            Err(e) => {
                eprintln!("gesprog: error aceptando conexión: {}", e);
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
        eprintln!("gesprog: Terminar recibido, saliendo");
        process::exit(0);
    }
}

fn procesar(svc: &Servicio, mensaje: &str) -> (String, bool) {
    let raiz: PeticionRaiz = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(_) => return (error("operacion desconocida"), false),
    };
    if raiz.servicio != "gesprog" {
        return (error("servicio desconocido"), false);
    }

    let op: OpGesprog = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(e) => {
            // Falta de "ejecutable" en Guardar es el caso típico
            let msg = e.to_string();
            if msg.contains("ejecutable") {
                return (error("falta campo: ejecutable"), false);
            }
            return (error("operacion desconocida"), false);
        }
    };

    let estado_actual = *svc.estado.lock().unwrap();

    match op {
        OpGesprog::Suspender => match estado_actual.suspender() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                (ok(), false)
            }
            Err(m) => (error(m), false),
        },
        OpGesprog::Reasumir => match estado_actual.reasumir() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                (ok(), false)
            }
            Err(m) => (error(m), false),
        },
        OpGesprog::Terminar => match estado_actual.terminar() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                (ok(), true)
            }
            Err(m) => (error(m), false),
        },
        // Leer está permitido en Suspendido (figura 4 del PDF).
        OpGesprog::Leer { id_programa } => (leer(svc, id_programa), false),

        _ if !estado_actual.esta_corriendo() => (error("servicio suspendido"), false),

        OpGesprog::Guardar {
            ejecutable,
            args,
            env,
        } => (guardar(svc, &ejecutable, args, env), false),
        OpGesprog::Actualizar { id_programa, ruta } => {
            (actualizar(svc, &id_programa, &ruta), false)
        }
        OpGesprog::Borrar { id_programa } => (borrar(svc, &id_programa), false),
    }
}

fn guardar(
    svc: &Servicio,
    ejecutable: &str,
    args: Vec<String>,
    env: Vec<String>,
) -> String {
    let fuente = Path::new(ejecutable);
    if !fuente.exists() {
        return error("no se pudo guardar el programa");
    }
    let id = svc.gen.siguiente();
    let nombre = fuente
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| id.clone());

    let destino = aralmac::ruta_bin_programa(&svc.aralmac, &id);
    if let Some(p) = destino.parent() {
        let _ = fs::create_dir_all(p);
    }
    if fs::copy(fuente, &destino).is_err() {
        return error("no se pudo guardar el programa");
    }

    let meta = MetadatosPrograma {
        id_programa: id.clone(),
        nombre,
        args,
        env,
        ruta_aralmac: destino.to_string_lossy().into_owned(),
    };
    let json_meta = match serde_json::to_string(&serde_json::json!({
        "id-programa": meta.id_programa,
        "nombre": meta.nombre,
        "args": meta.args,
        "env": meta.env,
        "ruta_aralmac": meta.ruta_aralmac,
    })) {
        Ok(s) => s,
        Err(_) => return error("no se pudo guardar el programa"),
    };
    let ruta_meta = aralmac::ruta_meta_programa(&svc.aralmac, &id);
    if fs::write(&ruta_meta, json_meta).is_err() {
        let _ = fs::remove_file(&destino);
        return error("no se pudo guardar el programa");
    }

    ok_con(&[("id-programa", Value::String(id))])
}

fn leer(svc: &Servicio, id: Option<String>) -> String {
    match id {
        Some(id) => {
            let ruta_meta = aralmac::ruta_meta_programa(&svc.aralmac, &id);
            if !ruta_meta.exists() {
                return error("programa no encontrado");
            }
            let contenido = match fs::read_to_string(&ruta_meta) {
                Ok(c) => c,
                Err(_) => return error("programa no encontrado"),
            };
            let mut v: Value = match serde_json::from_str(&contenido) {
                Ok(v) => v,
                Err(_) => return error("programa no encontrado"),
            };
            // No exponer ruta interna al cliente.
            if let Some(obj) = v.as_object_mut() {
                obj.remove("ruta_aralmac");
            }
            ok_con(&[("programa", v)])
        }
        None => match aralmac::listar_programas(&svc.aralmac) {
            Ok(lista) => ok_con(&[("programas", json!(lista))]),
            Err(_) => error("error al listar programas"),
        },
    }
}

fn actualizar(svc: &Servicio, id: &str, ruta_fuente: &str) -> String {
    let ruta_meta = aralmac::ruta_meta_programa(&svc.aralmac, id);
    let ruta_bin = aralmac::ruta_bin_programa(&svc.aralmac, id);
    if !ruta_meta.exists() {
        return error("programa no encontrado");
    }
    if !Path::new(ruta_fuente).exists() {
        return error("no se pudo actualizar el programa");
    }
    if fs::copy(ruta_fuente, &ruta_bin).is_err() {
        return error("no se pudo actualizar el programa");
    }
    // Actualizar campo "nombre" de los metadatos al nombre del nuevo binario.
    let contenido = match fs::read_to_string(&ruta_meta) {
        Ok(c) => c,
        Err(_) => return error("no se pudo actualizar el programa"),
    };
    let mut v: Value = match serde_json::from_str(&contenido) {
        Ok(v) => v,
        Err(_) => return error("no se pudo actualizar el programa"),
    };
    let nuevo_nombre = Path::new(ruta_fuente)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(obj) = v.as_object_mut() {
        obj.insert("nombre".into(), Value::String(nuevo_nombre));
    }
    if fs::write(&ruta_meta, v.to_string()).is_err() {
        return error("no se pudo actualizar el programa");
    }
    ok()
}

fn borrar(svc: &Servicio, id: &str) -> String {
    let ruta_meta = aralmac::ruta_meta_programa(&svc.aralmac, id);
    let ruta_bin = aralmac::ruta_bin_programa(&svc.aralmac, id);
    if !ruta_meta.exists() {
        return error("programa no encontrado");
    }
    let _ = fs::remove_file(&ruta_bin);
    if fs::remove_file(&ruta_meta).is_err() {
        return error("programa no encontrado");
    }
    ok()
}
