//! Servicio ejecutor — Ejecución de procesos de lotes.
//!
//! Implementa la sección 3.11 del PDF de la práctica.
//! Operaciones: Ejecutar / Estado / Matar / Suspender / Reasumir / Parar.

use clap::Parser;
use practica::aralmac;
use practica::estado::EstadoEjecutor;
use practica::ids::Generador;
use practica::ipc::{crear_listener, ListenerExt, Sesion};
use practica::protocolo::{error, ok, ok_con, OpEjecutor, PeticionRaiz};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, process, thread};

/// ejecutor -e <tuberia-nombrada> [-d <tuberia-nombrada>] -x <info-aralmac>
#[derive(Parser, Debug)]
#[command(name = "ejecutor", about = "Ejecutor de procesos de lotes")]
struct Cli {
    #[arg(short = 'e')]
    e: String,
    #[arg(short = 'd')]
    d: Option<String>,
    #[arg(short = 'x')]
    x: PathBuf,
}

/// Información de un proceso por lotes lanzado por el servicio.
struct ProcesoInfo {
    id_ejecucion: String,
    id_programa: String,
    hijo: Option<Child>,
    estado: String, // "Ejecutando" | "Suspendido" | "Terminado"
    codigo_salida: Option<i32>,
}

struct Servicio {
    estado: Mutex<EstadoEjecutor>,
    gen: Generador,
    aralmac: PathBuf,
    procesos: Mutex<HashMap<String, ProcesoInfo>>,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = fs::create_dir_all(&cli.x) {
        eprintln!("ejecutor: no se pudo crear aralmac {}: {}", cli.x.display(), e);
        process::exit(1);
    }

    let listener = match crear_listener(&cli.e) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ejecutor: no se pudo abrir la tuberia {}: {}", cli.e, e);
            process::exit(1);
        }
    };

    let svc = Arc::new(Servicio {
        estado: Mutex::new(EstadoEjecutor::Ejecutar),
        gen: Generador::nuevo("e"),
        aralmac: cli.x.clone(),
        procesos: Mutex::new(HashMap::new()),
    });

    eprintln!(
        "ejecutor: escuchando en {} (aralmac={})",
        cli.e,
        cli.x.display()
    );

    // Reaper: sondea hijos para detectar terminaciones y ejecuta Parar cuando procede.
    {
        let svc = Arc::clone(&svc);
        thread::spawn(move || reaper(svc));
    }

    for conexion in listener.incoming() {
        let conexion = match conexion {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ejecutor: error aceptando conexión: {}", e);
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

    let respuesta = procesar(&svc, &mensaje);
    let _ = sesion.escribir_mensaje(&respuesta);
}

fn procesar(svc: &Servicio, mensaje: &str) -> String {
    let raiz: PeticionRaiz = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(_) => return error("operacion desconocida"),
    };
    if raiz.servicio != "ejecutor" {
        return error("servicio desconocido");
    }

    let op: OpEjecutor = match serde_json::from_str(mensaje) {
        Ok(v) => v,
        Err(e) => {
            let m = e.to_string();
            if m.contains("id-programa") {
                return error("falta campo: id-programa");
            }
            if m.contains("id-ejecucion") {
                return error("falta campo: id-ejecucion");
            }
            return error("operacion desconocida");
        }
    };

    let estado_actual = *svc.estado.lock().unwrap();

    match op {
        OpEjecutor::Suspender => match estado_actual.suspender() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                suspender_todos(svc);
                ok()
            }
            Err(m) => error(m),
        },
        OpEjecutor::Reasumir => match estado_actual.reasumir() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                reasumir_todos(svc);
                ok()
            }
            Err(m) => error(m),
        },
        OpEjecutor::Parar => match estado_actual.parar() {
            Ok(e) => {
                *svc.estado.lock().unwrap() = e;
                ok()
            }
            Err(m) => error(m),
        },
        OpEjecutor::Estado { id_ejecucion } => estado(svc, id_ejecucion),
        OpEjecutor::Matar { id_ejecucion } => matar(svc, &id_ejecucion),
        OpEjecutor::Ejecutar {
            id_programa,
            stdin,
            stdout,
            stderr,
        } => {
            if estado_actual == EstadoEjecutor::Suspendidos {
                return error("servicio suspendido");
            }
            if !estado_actual.acepta_nuevos() {
                return error("servicio parando");
            }
            ejecutar(svc, &id_programa, stdin, stdout, stderr)
        }
    }
}

fn ejecutar(
    svc: &Servicio,
    id_programa: &str,
    stdin_id: Option<String>,
    stdout_id: Option<String>,
    stderr_id: Option<String>,
) -> String {
    let ruta_meta = aralmac::ruta_meta_programa(&svc.aralmac, id_programa);
    if !ruta_meta.exists() {
        return error("no se pudo ejecutar el programa");
    }
    let contenido = match fs::read_to_string(&ruta_meta) {
        Ok(c) => c,
        Err(_) => return error("no se pudo ejecutar el programa"),
    };
    let meta: Value = match serde_json::from_str(&contenido) {
        Ok(v) => v,
        Err(_) => return error("no se pudo ejecutar el programa"),
    };
    let ruta_bin = match meta.get("ruta_aralmac").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => aralmac::ruta_bin_programa(&svc.aralmac, id_programa)
            .to_string_lossy()
            .into_owned(),
    };
    let args: Vec<String> = meta
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let envs: Vec<(String, String)> = meta
        .get("env")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| s.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let mut cmd = Command::new(&ruta_bin);
    cmd.args(&args);
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    if let Some(id) = stdin_id {
        match fs::File::open(aralmac::ruta_fichero(&svc.aralmac, &id)) {
            Ok(f) => {
                cmd.stdin(Stdio::from(f));
            }
            Err(_) => return error("no se pudo ejecutar el programa"),
        }
    } else {
        cmd.stdin(Stdio::null());
    }
    if let Some(id) = stdout_id {
        match fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(aralmac::ruta_fichero(&svc.aralmac, &id))
        {
            Ok(f) => {
                cmd.stdout(Stdio::from(f));
            }
            Err(_) => return error("no se pudo ejecutar el programa"),
        }
    } else {
        cmd.stdout(Stdio::null());
    }
    if let Some(id) = stderr_id {
        match fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(aralmac::ruta_fichero(&svc.aralmac, &id))
        {
            Ok(f) => {
                cmd.stderr(Stdio::from(f));
            }
            Err(_) => return error("no se pudo ejecutar el programa"),
        }
    } else {
        cmd.stderr(Stdio::null());
    }

    let hijo = match cmd.spawn() {
        Ok(h) => h,
        Err(_) => return error("no se pudo ejecutar el programa"),
    };

    let id_ej = svc.gen.siguiente();
    let info = ProcesoInfo {
        id_ejecucion: id_ej.clone(),
        id_programa: id_programa.to_string(),
        hijo: Some(hijo),
        estado: "Ejecutando".into(),
        codigo_salida: None,
    };
    svc.procesos.lock().unwrap().insert(id_ej.clone(), info);

    ok_con(&[("id-ejecucion", Value::String(id_ej))])
}

fn estado(svc: &Servicio, id_ejecucion: Option<String>) -> String {
    let mut procesos = svc.procesos.lock().unwrap();
    actualizar_terminados(&mut procesos);

    match id_ejecucion {
        Some(id) => match procesos.get(&id) {
            Some(p) => proceso_a_json_ok(p),
            None => error("proceso no encontrado"),
        },
        None => {
            let lista: Vec<Value> = procesos.values().map(proceso_a_json_obj).collect();
            ok_con(&[("procesos", Value::Array(lista))])
        }
    }
}

fn matar(svc: &Servicio, id_ejecucion: &str) -> String {
    let mut procesos = svc.procesos.lock().unwrap();
    actualizar_terminados(&mut procesos);

    let info = match procesos.get_mut(id_ejecucion) {
        Some(p) => p,
        None => return error("proceso no encontrado o ya terminado"),
    };
    if info.estado == "Terminado" {
        return error("proceso no encontrado o ya terminado");
    }
    if let Some(h) = info.hijo.as_mut() {
        let _ = h.kill();
        if let Ok(status) = h.wait() {
            info.codigo_salida = status.code();
        }
    }
    info.estado = "Terminado".into();
    ok()
}

fn proceso_a_json_obj(p: &ProcesoInfo) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "id-ejecucion".into(),
        Value::String(p.id_ejecucion.clone()),
    );
    obj.insert("id-programa".into(), Value::String(p.id_programa.clone()));
    obj.insert("proceso-estado".into(), Value::String(p.estado.clone()));
    if let Some(c) = p.codigo_salida {
        obj.insert("codigo-salida".into(), Value::from(c));
    }
    Value::Object(obj)
}

fn proceso_a_json_ok(p: &ProcesoInfo) -> String {
    let obj = match proceso_a_json_obj(p) {
        Value::Object(o) => o,
        _ => return error("proceso no encontrado"),
    };
    let mut m = serde_json::Map::new();
    m.insert("estado".into(), Value::String("ok".into()));
    for (k, v) in obj {
        m.insert(k, v);
    }
    Value::Object(m).to_string()
}

fn actualizar_terminados(procesos: &mut HashMap<String, ProcesoInfo>) {
    for info in procesos.values_mut() {
        if info.estado == "Terminado" {
            continue;
        }
        if let Some(h) = info.hijo.as_mut() {
            match h.try_wait() {
                Ok(Some(status)) => {
                    info.estado = "Terminado".into();
                    info.codigo_salida = status.code();
                }
                Ok(None) => { /* sigue activo */ }
                Err(_) => {
                    info.estado = "Terminado".into();
                }
            }
        }
    }
}

fn suspender_todos(svc: &Servicio) {
    let mut procesos = svc.procesos.lock().unwrap();
    for info in procesos.values_mut() {
        if info.estado != "Ejecutando" {
            continue;
        }
        if let Some(h) = info.hijo.as_ref() {
            if suspender_proceso(h.id()).is_ok() {
                info.estado = "Suspendido".into();
            }
        }
    }
}

fn reasumir_todos(svc: &Servicio) {
    let mut procesos = svc.procesos.lock().unwrap();
    for info in procesos.values_mut() {
        if info.estado != "Suspendido" {
            continue;
        }
        if let Some(h) = info.hijo.as_ref() {
            if reasumir_proceso(h.id()).is_ok() {
                info.estado = "Ejecutando".into();
            }
        }
    }
}

fn reaper(svc: Arc<Servicio>) {
    loop {
        thread::sleep(Duration::from_millis(200));
        let mut procesos = svc.procesos.lock().unwrap();
        actualizar_terminados(&mut procesos);
        let activos = procesos
            .values()
            .filter(|p| p.estado != "Terminado")
            .count();
        let estado_global = *svc.estado.lock().unwrap();
        drop(procesos);
        if estado_global == EstadoEjecutor::Parar && activos == 0 {
            eprintln!("ejecutor: Parar - sin procesos activos, saliendo");
            process::exit(0);
        }
    }
}

// ---------------------------------------------------------------------------
// Suspender / Reasumir procesos hijos. Implementación dependiente del SO.
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn suspender_proceso(pid: u32) -> std::io::Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGSTOP)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(unix)]
fn reasumir_proceso(pid: u32) -> std::io::Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGCONT)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(windows)]
fn suspender_proceso(_pid: u32) -> std::io::Result<()> {
    // Windows: la suspensión real requiere NtSuspendProcess (no expuesta en stdlib).
    // Marcamos solo el estado interno; los hijos siguen corriendo.
    eprintln!("ejecutor: aviso, Suspender no es real en Windows (sólo estado lógico)");
    Ok(())
}

#[cfg(windows)]
fn reasumir_proceso(_pid: u32) -> std::io::Result<()> {
    Ok(())
}
