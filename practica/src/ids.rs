//! Generación de identificadores únicos `f-XXXX`, `p-XXXX`, `e-XXXX`.
//!
//! El PDF (sección 3.8.3) define el formato como `<prefijo>-XXXX`. El
//! contador es por servicio y se reinicia al arrancar el proceso (no se
//! requiere persistencia en el enunciado).

use std::sync::atomic::{AtomicU32, Ordering};

/// Generador de IDs basado en un contador atómico thread-safe.
pub struct Generador {
    prefijo: &'static str,
    contador: AtomicU32,
}

impl Generador {
    pub const fn nuevo(prefijo: &'static str) -> Self {
        Self {
            prefijo,
            contador: AtomicU32::new(1),
        }
    }

    /// Devuelve el siguiente identificador, p.ej. "f-0001", "f-0002".
    pub fn siguiente(&self) -> String {
        let n = self.contador.fetch_add(1, Ordering::SeqCst);
        format!("{}-{:04}", self.prefijo, n)
    }

    /// Reposiciona el contador (útil al cargar estado existente de aralmac).
    pub fn ajustar_minimo(&self, valor: u32) {
        let mut actual = self.contador.load(Ordering::SeqCst);
        while actual < valor {
            match self.contador.compare_exchange_weak(
                actual,
                valor,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return,
                Err(v) => actual = v,
            }
        }
    }
}

/// Extrae el número de un identificador `prefijo-NNNN`.
pub fn numero(id: &str) -> Option<u32> {
    let (_p, n) = id.split_once('-')?;
    n.parse().ok()
}
