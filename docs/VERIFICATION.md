# Evidencia 0.9.0-alpha.1

21 de noviembre de 2026. Mac Apple M2 con 8 GB, Node 24, Theia 1.77.0 y Electron 44.9.1.

## Aprobado localmente

- Compilación TypeScript y bundles browser/node/electron. Las 75 pruebas se repitieron bajo el Node del Electron distribuido, también sin fallos ni omisiones. El paquete aprobó transacciones Rust, worker de grafo con Python y PTY. El DMG HFS+ pasó hdiutil verify.
- Los módulos de conexión reales de Theia y la fábrica Ollama decorada se cargaron juntos y conservaron identidad/modelo correctamente (comprobación adicional de integración de dependencias).
- Auditoría OSV de engine/Cargo.lock: 64 paquetes de registro, cero hallazgos conocidos; excluye bibliotecas nativas, SO y el código legacy.
- 75 pruebas de unidad/integración local, 75 aprobadas, cero omitidas. Incluyen grupos transaccionales, backend, límites de agentes, cancelación, graph multilenguaje/incremental, lifecycle GitHub, updates Ed25519 y regresiones de dependencias.
- Dos pruebas reales de protocolo: TypeScript del plugin incluido (diagnósticos, completado, referencias, rename y codefix) y servidor DAP Node oficial 1.105.0 (breakpoints, stack, variables, evaluación y step). El asset standalone DAP está fijado por SHA256 en scripts/prepare-qa.mjs. No confundir con E2E del host gráfico de plugins.
- Flujo Git remoto real con GitService en repositorio de prueba clonado: commit, rama, push/fetch/pull, crear/listar/cerrar PR #1 en Juanexgo/ForgeIDE. Rama eliminada, PR no fusionado. Cuenta gh existente reutilizada, desconexión/reconexión solo dentro de ForgeIDE.
- Auditoría NPM de lockfile: cero alertas conocidas de todas las severidades al ejecutarla. KaTeX, jsdiff y dependencias modernas de glob/minimatch actualizadas con selectores específicos; se conservan APIs antiguas en otros consumidores. Pruebas regresivas verifican compatibilidad. docs/dependency-audit.json contiene alcance y resumen.
- Inventario CycloneDX de 1187 componentes NPM; es inventario plano, no certificación de licencias ni de los binarios distribuidos.

## Evidencia pública y límites

Actions: https://github.com/Juanexgo/ForgeIDE/actions

La ejecución final https://github.com/Juanexgo/ForgeIDE/actions/runs/44879776714, commit 8553f1e, aprobó macOS, Windows y Linux. Windows: 75 pruebas, tres crash recovery y dos de protocolos, todas aprobadas sin omisiones. La normalización de rutas corrigió el breakpoint; después se corrigió el cierre antes de eliminar fixtures. Los intentos anteriores fallidos permanecen visibles. El packaging de Linux AppImage/DEB aprobó en 44878027543; Windows NSIS y macOS ZIP aprobaron sus jobs de 44876977785.

El benchmark de grafo genera 100/1000/10000 archivos, informa truncado, tres muestras y una secuencia incremental cold/no-op/edición. Es una prueba del analizador, no de latencia del editor, arranque completo o superioridad frente a competidores. Las métricas no equivalen a garantías de tiempo real.

## Pendiente o fallido, sin convertirlo en éxito

- La herramienta visual sigue detectando bloqueo del Mac. No se certifica accesibilidad, instalación visual de extensiones ni recorrido completo de depuración desde botones.
- Ollama local produjo una propuesta incompleta en un ensayo; los demás ensayos no aprobaron el recorrido completo. Hay límites de transporte y herramientas, pero eso no certifica calidad de los cuatro roles ni tareas autónomas generales.
- Docker tuvo arranques/paradas bloqueados. No se borraron imágenes ni volúmenes del usuario. La configuración de memoria permanece en 5120 MiB. No presentar un daemon sin responder como resultado positivo.
- Las cinco integraciones reales Docker se repitieron y aprobaron con 0.9: Node single-file, grupos Node/Python y dependencias públicas fijadas Node/Python. Comprobaron pruebas reales, proof, apply/revert y rechazo de inputs/locks posteriores.
- El ensayo Qwen3:4b alcanzó el límite de generación tanto con 384 como con 1024 tokens; un ensayo Llama no leyó archivos y otro propuso un fragmento incompleto. Ninguno se cuenta como tarea IA aprobada. Docker se detuvo durante generación y se volvió a iniciar para el resto de pruebas; no se alteró la memoria de 5120 MiB.
- No se dispone de certificado Developer ID de distribución ni certificado de firma Windows. La firma Ed25519 del feed autentica metadatos y hashes; no reemplaza firma/notarización del sistema operativo.
- Fresh OAuth y token realmente expirado requieren un flujo real de autorización del usuario. Las pruebas deterministas de expiración no son esa aprobación.

## Publicación y actualizaciones

La release v0.9.0-alpha.1 contiene DMG macOS ARM64, NSIS Windows x64, AppImage/DEB Linux x64 y código. Los cuatro tamaños y SHA256 se compararon con los digests devueltos por GitHub. El feed público updates-alpha pasó la comprobación real HTTPS, firma Ed25519 y versiones con UpdateService de producción: 0.8.1 vio una actualización y 0.9.0 quedó al día. No se ejecutaron instaladores durante esa prueba. Ver benchmarks/update-public-feed.json y los límites de firma del SO arriba.

## Revisión 0.9.1 — 23 de noviembre de 2026

- `yarn build:extension` y `yarn typecheck`: aprobados.
- `yarn test`: 91 pruebas aprobadas, cero fallos/omisiones.
- `yarn test:protocol`: dos protocolos reales aprobados (TypeScript y Node DAP).
- `node scripts/audit-gate.mjs`: cero alertas NPM conocidas; SBOM de lockfile regenerado.
- `yarn test:agents`: cuatro fixtures con el modelo real Qwen3 Instruct, contratos de herramientas compartidos y Docker. Consulte el recibo JSON para la ejecución exacta; la prueba de mutación de ForgeTests debe fallar deliberadamente.
- `yarn benchmark:search`: comparación de los motores CLI incluidos en las tres aplicaciones; no se presenta como benchmark del IDE completo.
- `tests/accessibility.test.cjs`: seis componentes iniciales pasan comprobaciones semánticas axe. No certifica contraste, interacciones, lector de pantalla, IME ni accesibilidad del shell Electron.
- `tests/signing.test.cjs`: configuración y rechazo de credenciales ausentes. No equivale a firmar una aplicación.

Para publicar nuevos instaladores, ejecutar `checks.yml` y `package-preview.yml` en el mismo commit. `publish-qualified-preview.yml` comprueba procedencia y exige los tres sistemas; genera una release draft sin firma de distribución. Una ejecución previa de otra versión no es suficiente.

La ejecución pública 45280832573 aprobó los tres sistemas (91 unitarias/componentes, tres de recuperación y dos protocolos en cada uno, cero omisiones). Packaging 45280860635 y colector 45282761227 aprobaron el mismo commit. Se verificó una autorización GitHub nueva y el rechazo real de una credencial sintética inválida (401), sin revocar el token del usuario. La UI macOS 0.9.1 mostró el diagnóstico TypeScript y su desaparición al deshacer, así como sugerencias de console. Depuración/extensiones gráficas y accesibilidad completa siguen pendientes.
