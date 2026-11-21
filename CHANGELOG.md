# 0.8.1-alpha.1

- Perfiles de dependencias Node/Python: manifiestos validados, consentimiento ligado a digest, builder Docker con contexto mínimo, runtime inmutable y pruebas offline; no reusa credenciales ni node_modules del host.
- Rechazo de imágenes no registradas, planes obsoletos, paquetes locales/privados, integridades débiles y source builds de Python.
- Presupuestos separados de arranque del contenedor (120s) y ejecución de tests (60s), con tiempos explícitos en recibos y rechazo seguro si el arranque falla.
- Cancelación del proceso cliente fuerza cierre después de dos segundos de SIGTERM.
- 54 tests locales aprobados y scripts explícitos de integración Docker/dependencias y evaluación Ollama real. Resultados reales en VERIFICATION.md: un script incluido no implica una prueba aprobada.
- Comprobados cuenta GitHub existente, inicialización Git, escritura y guardado desde la UI. No hubo OAuth nuevo, publicación ni modelos externos.

# 0.8.0-alpha.1

- Motor Rust para grupos create/edit/delete, rollback y recuperación de proceso; bloqueo de kernel y temporales rastreados.
- SafeApply multiarchivo, revisión/selectores por archivo, controles de buffers entre ventanas y verificación exacta de inputs.
- Node/Python aislados en Docker con recibos SQLite y herramientas de agentes con permiso.
- Panel Git con stage por fragmento, ramas, merge/conflictos revisables, remotos, worktrees, PR y login web de GitHub CLI.
- Grafo JS/TS con alias/configs/paquetes locales, worker separado y vista SVG de dependencias.
- Inventario y scanner de dependencias npm/OSV con consentimiento y detector de herramientas de lenguaje.
- Correcciones de dependencias: auditoría del lockfile sin findings altos/críticos, moderados/bajos pendientes.
- Perfil Electron seleccionado antes del lock de instancia y sidecar/worker incluidos en distribución.
- 47 tests locales + 3 crash tests + 3 integraciones Docker reales aprobados en macOS arm64.

Limitaciones y puertas de beta: docs/ESTADO-COMPLETO.md. Esta versión continúa siendo alpha.
