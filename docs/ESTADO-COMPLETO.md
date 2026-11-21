# Estado verificable de ForgeIDE 0.9.0-alpha.1

21 de noviembre de 2026. Esta alpha implementa mejoras reales, pero no certifica paridad ni superioridad universal frente a otros IDE. Una función integrada no equivale a un flujo completo validado.

| Área | Implementación y evidencia actual | Lo que no está aprobado |
| --- | --- | --- |
| IDE | Theia/Electron/Monaco, 90 plugins base, edición, terminal, Git, LSP/DAP. Servidor TypeScript incluido: diagnósticos, completado, referencias, rename y code fixes. DAP Node oficial de la misma versión que el plugin: breakpoint, stack, variables, watch y step | E2E visual de todos los flujos, IME, lector de pantalla y escalas. La herramienta visual informa Mac bloqueado incluso después del aviso de desbloqueo. La prueba DAP usa servidor standalone, no el host gráfico de extensiones |
| IA | Cuatro roles, lista cerrada de herramientas, presupuestos de tiempo/llamadas, cancelación durante carga HTTP, fallback JSON que solo propone cambios sobre archivos leídos. Propuestas y verificaciones no aplican cambios automáticamente | No hay tarea de modelo local aprobada de extremo a extremo. Llama produjo una propuesta incompleta en un ensayo; otros ensayos agotaron tiempo o no llegaron a Docker. Las pruebas unitarias de transporte no son evaluación de calidad del modelo |
| SafeApply | Motor Rust, SQLite, grupos create/edit/delete, baseline/proof, apply/revert y recuperación. Integraciones Docker previas Node/Python y dependencias fijadas | Las integraciones Docker de 0.8.1 son evidencia histórica hasta repetirse con la entrega actual. Un daemon detenido/bloqueado nunca cuenta como pruebas aprobadas |
| Graph | Tree-sitter para imports Python/Go/Rust; JS/TS con alias y paquetes; Cargo local, Go modules y servicios Compose. Caché incremental por identidad/metadata, cambios/borrados y nueva resolución de configuración, límites explícitos | No analiza semántica de macros Rust, cfg/build tags, imports dinámicos o servicios remotos en ejecución. Contains en Compose es relación de contexto conservadora. El límite sigue en 1000 archivos + 100 servicios; un mapa truncado es parcial |
| GitHub | Cuenta existente detectada; desconectar/reconectar ForgeIDE con estado persistente sin borrar credenciales gh. Expiración/cancelación probadas con procesos controlados. Flujo remoto real mediante GitService: branch, commit, push, fetch, pull, crear/listar/cerrar PR #1 y eliminar rama | Consentimiento OAuth nuevo en navegador y expiración de una credencial real. No se revocaron credenciales compartidas para una prueba |
| Distribución | Recetas macOS/Windows/Linux, CI y packaging públicos. Verificación Ed25519 de feed, protección contra downgrade/replay, descarga con hash/tamaño y URL permitida, cancelación y limpieza parcial | Firma Developer ID/notarización de Apple y firma Windows requieren certificados de distribución que no están disponibles. La descarga es manual y nunca ejecuta el instalador. El feed remoto necesita publicarse con artefactos reales antes de funcionar |
| Calidad | 75/75 pruebas locales sin omisiones; dos pruebas reales de protocolo; auditoría NPM completa sin alertas conocidas; SBOM de lockfile; CI pública y benchmark incremental | Accesibilidad completa, pruebas de jornadas reales y comparación medible del editor con VS Code/Cursor. El inventario y la auditoría no incluyen todos los binarios, VSIX, Cargo ni bibliotecas del SO |

## CI pública

Repositorio: https://github.com/Juanexgo/ForgeIDE

- Primera ejecución: macOS y Linux aprobados, Windows detectó problemas de rutas cortas y cierre de fixtures SQLite.
- Segunda ejecución: macOS y Linux aprobados; quedó la comparación de rutas cortas de Windows.
- Se corrigió la identidad de directorios mediante identificadores del sistema de archivos, con ruta nativa como alternativa. La ejecución posterior debe consultarse en Actions; no se presume aprobada antes de que termine.
- Los symlinks se intentan también en Windows y solo se omiten si el runner carece del permiso concreto. Las omisiones deben leerse en el log.

## Límites de seguridad y publicación

Las extensiones Node compatibles tienen los privilegios del usuario: no son un sandbox. Los agentes Forge no reciben shell ni aplicación directa. Docker verifica snapshots sin red y con recursos limitados; no constituye protección absoluta del host. No se incluyeron claves de proveedores ni la clave privada de firma del feed en el código público.

Los documentos anteriores de 0.8.1 describen evidencia de esa entrega. Este documento y VERIFICATION.md definen el estado actual; no trasladar una aprobación entre versiones sin repetir el caso correspondiente.
