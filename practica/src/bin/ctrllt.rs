//! ctrllt — Pasarela del sistema.
//!
//! Implementa la sección 3.12 del PDF de la práctica.
//!
//! Funcionamiento:
//!   1. Lee una petición JSON del cliente.
//!   2. Mira el campo "servicio".
//!   3. Si vale "gesfich"/"gesprog"/"ejecutor": reenvía la petición a la
//!      tubería correspondiente, lee la respuesta y la devuelve al cliente
//!      tal cual.
//!   4. Si vale "ctrllt": maneja localmente la única operación permitida
//!      del controlador, "Terminar".
//!
//! Nota sobre las opciones del PDF: el enunciado define `-c` dos veces
//! (para ctrllt y para gesprog). Para evitar el conflicto se mantienen
//! los flags cortos de petición tal como pide el PDF (-c, -f, -p, -e) y
//! se ofrecen los de respuesta como flags largos (--resp-*). Los flags
//! de respuesta son opcionales y no se usan en la IPC full-duplex.

use clap::Parser;
use practica::ipc::{conectar, crear_listener, solicitar, ListenerExt, Sesion};
use practica::protocolo::{error, ok, PeticionRaiz};
use std::sync::Arc;
use std::{process, thread};

#[derive(Parser, Debug)]
#[command(name = "ctrllt", about = "Pasarela del sistema ejecutor de lotes")]
struct Cli {
    /// Tubería para recibir peticiones de los clientes.
    #[arg(short = 'c')]
    c: String,
    /// Tubería de respuesta a clientes (sólo half-duplex).
    #[arg(long = "resp-ctrllt")]
    a: Option<String>,

    /// Tubería de peticiones a gesfich.
    #[arg(short = 'f')]
    f: String,
    #[arg(long = "resp-gesfich")]
    b: Option<String>,

    /// Tubería de peticiones a gesprog.
    #[arg(short = 'p')]
    p: String,
    #[arg(long = "resp-gesprog")]
    g: Option<String>,

    /// Tubería de peticiones a ejecutor.
    #[arg(short = 'e')]
    e: String,
    #[arg(long = "resp-ejecutor")]
    d: Option<String>,
}

struct Rutas {
    gesfich: String,
    gesprog: String,
    ejecutor: String,
}

fn main() {
    let cli = Cli::parse();

    let listener = match crear_listener(&cli.c) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ctrllt: no se pudo abrir la tuberia {}: {}", cli.c, e);
            process::exit(1);
        }
    };

    let rutas = Arc::new(Rutas {
        gesfich: cli.f.clone(),
        gesprog: cli.p.clone(),
        ejecutor: cli.e.clone(),
    });

    eprintln!("ctrllt: escuchando en {}", cli.c);

    for conexion in listener.incoming() {
        let conexion = match conexion {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ctrllt: error aceptando conexión: {}", e);
                continue;
            }
        };
        let rutas = Arc::clone(&rutas);
        thread::spawn(move || atender(rutas, conexion));
    }
}

fn atender(rutas: Arc<Rutas>, stream: interprocess::local_socket::Stream) {
    let mut sesion = Sesion::nueva(stream);
    let mensaje = match sesion.leer_mensaje() {
        Ok(Some(m)) => m,
        _ => return,
    };

    let (respuesta, terminar) = enrutar(&rutas, &mensaje);
    let _ = sesion.escribir_mensaje(&respuesta);

    if terminar {
        eprintln!("ctrllt: Terminar global completo, saliendo");
        process::exit(0);
    }
}

/// Devuelve (respuesta, debe_terminar_proceso).
fn enrutar(rutas: &Rutas, mensaje: &str) -> (String, bool) {
    let raiz: PeticionRaiz = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(_) => return (error("operacion ctrllt desconocida"), false),
    };

    match raiz.servicio.as_str() {
        "ctrllt" => match raiz.operacion.as_str() {
            "Terminar" => (terminar_sistema(rutas), true),
            _ => (error("operacion ctrllt desconocida"), false),
        },
        "gesfich" => (
            reenviar(&rutas.gesfich, mensaje, "gesfich"),
            false,
        ),
        "gesprog" => (
            reenviar(&rutas.gesprog, mensaje, "gesprog"),
            false,
        ),
        "ejecutor" => (
            reenviar(&rutas.ejecutor, mensaje, "ejecutor"),
            false,
        ),
        _ => (error("servicio desconocido"), false),
    }
}

fn reenviar(tuberia_destino: &str, mensaje: &str, nombre_svc: &str) -> String {
    // Verifica que el servicio esté arriba.
    if conectar(tuberia_destino).is_err() {
        return error("servicio no conectado");
    }
    match solicitar(tuberia_destino, mensaje) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "ctrllt: error reenviando a {} ({}): {}",
                nombre_svc, tuberia_destino, e
            );
            error("error enviando solicitud al servicio")
        }
    }
}

/// Propaga el apagado a los tres servicios (sección 3.12.1 del PDF).
fn terminar_sistema(rutas: &Rutas) -> String {
    let _ = solicitar(
        &rutas.gesfich,
        r#"{"servicio":"gesfich","operacion":"Terminar"}"#,
    );
    let _ = solicitar(
        &rutas.gesprog,
        r#"{"servicio":"gesprog","operacion":"Terminar"}"#,
    );
    let _ = solicitar(
        &rutas.ejecutor,
        r#"{"servicio":"ejecutor","operacion":"Parar"}"#,
    );
    ok()
}
