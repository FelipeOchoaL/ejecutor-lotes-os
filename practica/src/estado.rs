//! Máquinas de estados de los tres servicios.
//!
//! Definidas en las figuras 3 (gesfich), 4 (gesprog) y 5 (ejecutor) del PDF.

/// Estado común a gesfich y gesprog.
///
///   inicio → Corriendo ⇄ Suspendido → Terminado
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoServicio {
    Corriendo,
    Suspendido,
    Terminado,
}

impl EstadoServicio {
    pub fn suspender(self) -> Result<Self, &'static str> {
        match self {
            EstadoServicio::Corriendo => Ok(EstadoServicio::Suspendido),
            _ => Err("transicion invalida"),
        }
    }

    pub fn reasumir(self) -> Result<Self, &'static str> {
        match self {
            EstadoServicio::Suspendido => Ok(EstadoServicio::Corriendo),
            _ => Err("transicion invalida"),
        }
    }

    pub fn terminar(self) -> Result<Self, &'static str> {
        match self {
            EstadoServicio::Terminado => Err("transicion invalida"),
            _ => Ok(EstadoServicio::Terminado),
        }
    }

    pub fn esta_corriendo(self) -> bool {
        matches!(self, EstadoServicio::Corriendo)
    }
}

/// Estado del servicio ejecutor (figura 5).
///
///   inicio → Ejecutar ⇄ Suspendidos → Parar (cuando /Proceso == 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoEjecutor {
    Ejecutar,
    Suspendidos,
    Parar,
}

impl EstadoEjecutor {
    pub fn suspender(self) -> Result<Self, &'static str> {
        match self {
            EstadoEjecutor::Ejecutar => Ok(EstadoEjecutor::Suspendidos),
            _ => Err("transicion invalida"),
        }
    }

    pub fn reasumir(self) -> Result<Self, &'static str> {
        match self {
            EstadoEjecutor::Suspendidos => Ok(EstadoEjecutor::Ejecutar),
            _ => Err("transicion invalida"),
        }
    }

    pub fn parar(self) -> Result<Self, &'static str> {
        match self {
            EstadoEjecutor::Parar => Err("transicion invalida"),
            _ => Ok(EstadoEjecutor::Parar),
        }
    }

    pub fn acepta_nuevos(self) -> bool {
        matches!(self, EstadoEjecutor::Ejecutar)
    }
}
