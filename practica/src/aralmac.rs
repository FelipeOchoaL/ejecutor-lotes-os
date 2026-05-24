//! Utilidades de almacenamiento sobre el directorio aralmac.
//!
//! Estructura física dentro de `<aralmac>`:
//!   aralmac/
//!     ficheros/
//!       f-0001         ← contenido binario
//!       f-0002
//!     programas/
//!       p-0001.json    ← metadatos: nombre, args, env, ruta_aralmac
//!       p-0001.bin     ← copia del ejecutable

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Devuelve la ruta del subdirectorio `ficheros/` y la crea si no existe.
pub fn dir_ficheros(base: &Path) -> io::Result<PathBuf> {
    let p = base.join("ficheros");
    fs::create_dir_all(&p)?;
    Ok(p)
}

/// Devuelve la ruta del subdirectorio `programas/` y la crea si no existe.
pub fn dir_programas(base: &Path) -> io::Result<PathBuf> {
    let p = base.join("programas");
    fs::create_dir_all(&p)?;
    Ok(p)
}

/// Ruta absoluta de un fichero `f-XXXX` dentro de aralmac.
pub fn ruta_fichero(base: &Path, id: &str) -> PathBuf {
    base.join("ficheros").join(id)
}

/// Ruta del JSON de metadatos para un programa `p-XXXX`.
pub fn ruta_meta_programa(base: &Path, id: &str) -> PathBuf {
    base.join("programas").join(format!("{}.json", id))
}

/// Ruta del binario copiado para un programa `p-XXXX`.
pub fn ruta_bin_programa(base: &Path, id: &str) -> PathBuf {
    base.join("programas").join(format!("{}.bin", id))
}

/// Lista los identificadores `f-XXXX` existentes en aralmac (ordenados).
pub fn listar_ficheros(base: &Path) -> io::Result<Vec<String>> {
    let dir = dir_ficheros(base)?;
    let mut ids = Vec::new();
    for entrada in fs::read_dir(dir)? {
        let entrada = entrada?;
        if let Some(nombre) = entrada.file_name().to_str() {
            if nombre.starts_with("f-") {
                ids.push(nombre.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

/// Lista los identificadores `p-XXXX` existentes (a partir de los .json).
pub fn listar_programas(base: &Path) -> io::Result<Vec<String>> {
    let dir = dir_programas(base)?;
    let mut ids = Vec::new();
    for entrada in fs::read_dir(dir)? {
        let entrada = entrada?;
        if let Some(nombre) = entrada.file_name().to_str() {
            if let Some(stem) = nombre.strip_suffix(".json") {
                if stem.starts_with("p-") {
                    ids.push(stem.to_string());
                }
            }
        }
    }
    ids.sort();
    Ok(ids)
}
