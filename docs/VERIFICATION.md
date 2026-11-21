# Evidencia 0.9.0-alpha.1

21 de noviembre de 2026. Mac Apple M2 con 8 GB, Node 24, Theia 1.77.0 y Electron 44.9.1.

## Aprobado localmente

- Compilación TypeScript y bundles browser/node/electron.
- 75 pruebas de unidad/integración local, 75 aprobadas, cero omitidas. Incluyen grupos transaccionales, backend, límites de agentes, cancelación, graph multilenguaje/incremental, lifecycle GitHub, updates Ed25519 y regresiones de dependencias.
- Dos pruebas reales de protocolo: TypeScript del plugin incluido (diagnósticos, completado, referencias, rename y codefix) y servidor DAP Node oficial 1.105.0 (breakpoints, stack, variables, evaluación y step). El asset standalone DAP está fijado por SHA256 en scripts/prepare-qa.mjs. No confundir con E2E del host gráfico de plugins.
- Flujo Git remoto real con GitService en repositorio de prueba clonado: commit, rama, push/fetch/pull, crear/listar/cerrar PR #1 en Juanexgo/ForgeIDE. Rama eliminada, PR no fusionado. Cuenta gh existente reutilizada, desconexión/reconexión solo dentro de ForgeIDE.
- Auditoría NPM de lockfile: cero alertas conocidas de todas las severidades al ejecutarla. KaTeX, jsdiff y dependencias modernas de glob/minimatch actualizadas con selectores específicos; se conservan APIs antiguas en otros consumidores. Pruebas regresivas verifican compatibilidad. docs/dependency-audit.json contiene alcance y resumen.
- Inventario CycloneDX de 1187 componentes NPM; es inventario plano, no certificación de licencias ni de los binarios distribuidos.

## Evidencia pública y límites

Actions: https://github.com/Juanexgo/ForgeIDE/actions

macOS y Linux pasaron las dos primeras ejecuciones de checks. Windows encontró fallos reales de rutas cortas y fixtures SQLite; se corrigieron y se lanzó otra revisión. Consulte el resultado del commit exacto antes de declarar aprobado Windows.

El benchmark de grafo genera 100/1000/10000 archivos, informa truncado, tres muestras y una secuencia incremental cold/no-op/edición. Es una prueba del analizador, no de latencia del editor, arranque completo o superioridad frente a competidores. Las métricas no equivalen a garantías de tiempo real.

## Pendiente o fallido, sin convertirlo en éxito

- La herramienta visual sigue detectando bloqueo del Mac. No se certifica accesibilidad, instalación visual de extensiones ni recorrido completo de depuración desde botones.
- Ollama local produjo una propuesta incompleta en un ensayo; los demás ensayos no aprobaron el recorrido completo. Hay límites de transporte y herramientas, pero eso no certifica calidad de los cuatro roles ni tareas autónomas generales.
- Docker tuvo arranques/paradas bloqueados. No se borraron imágenes ni volúmenes del usuario. La configuración de memoria permanece en 5120 MiB. No presentar un daemon sin responder como resultado positivo.
- SafeApply y perfiles con dependencias tienen cinco integraciones reales aprobadas de 0.8.1, no una repetición completa aprobada de 0.9.
- No se dispone de certificado Developer ID de distribución ni certificado de firma Windows. La firma Ed25519 del feed autentica metadatos y hashes; no reemplaza firma/notarización del sistema operativo.
- Fresh OAuth y token realmente expirado requieren un flujo real de autorización del usuario. Las pruebas deterministas de expiración no son esa aprobación.
