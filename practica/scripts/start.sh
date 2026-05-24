#!/usr/bin/env bash
# Arranca los cuatro servicios del sistema ctrllt en Linux/macOS.
#
# Uso:   ./scripts/start.sh
# Para parar: envía {"servicio":"ctrllt","operacion":"Terminar"} al pipe del cliente.

set -e

ARALMAC="${ARALMAC:-aralmac}"
PIPE_CTRLLT="${PIPE_CTRLLT:-pipe_ctrllt}"
PIPE_GESFICH="${PIPE_GESFICH:-pipe_gesfich}"
PIPE_GESPROG="${PIPE_GESPROG:-pipe_gesprog}"
PIPE_EJECUTOR="${PIPE_EJECUTOR:-pipe_ejecutor}"

PROFILE="${PROFILE:-debug}"
BIN="target/$PROFILE"

if [[ ! -x "$BIN/ctrllt" ]]; then
    echo "Compilando..."
    if [[ "$PROFILE" == "release" ]]; then cargo build --release; else cargo build; fi
fi

mkdir -p "$ARALMAC"

echo "Arrancando servicios (aralmac=$ARALMAC)..."
"$BIN/gesfich"  -f "$PIPE_GESFICH"  -x "$ARALMAC" &
"$BIN/gesprog"  -p "$PIPE_GESPROG"  -x "$ARALMAC" &
"$BIN/ejecutor" -e "$PIPE_EJECUTOR" -x "$ARALMAC" &
sleep 0.5
"$BIN/ctrllt" -c "$PIPE_CTRLLT" -f "$PIPE_GESFICH" -p "$PIPE_GESPROG" -e "$PIPE_EJECUTOR" &

echo "Sistema listo. Socket del cliente: $PIPE_CTRLLT (Unix domain socket)."
wait
