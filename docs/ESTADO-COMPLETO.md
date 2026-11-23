# Estado verificable de ForgeIDE 0.9.1-alpha.1

23 de noviembre de 2026. El producto continúa en alpha. Las pruebas siguientes acreditan flujos concretos; no certifican paridad completa ni superioridad universal frente a otros IDE.

| Área | Hecho y evidencia | Pendiente para cerrar la aceptación |
| --- | --- | --- |
| IDE | Theia/Electron/Monaco y 90 plugins base. LSP TypeScript real: diagnósticos, completado, referencias, rename y code fixes. DAP Node real: breakpoint, stack, variables, watch y step. Ambos protocolos repetidos localmente y en los tres sistemas. En la UI macOS 0.9.1 se comprobaron apertura, escritura, guardado, deshacer, diagnóstico TypeScript 2322 en Problems, limpieza del error al deshacer y sugerencias contextuales de console | Completar refactorización visual, extensiones, depuración gráfica, IME, lector de pantalla y escalas. La herramienta visual sufrió interrupciones de captura y reportes de bloqueo/entrada física; no se cuentan esos intentos como aprobaciones |
| IA | Los cuatro roles ForgeImplement, ForgeFix, ForgeTests y ForgeSecurity aprobaron fixtures reales con Qwen3 Instruct 2507, herramientas de producción, propuesta persistida, Docker, aplicación y reversión. ForgeTests detectó una mutación deliberada. El turno termina al guardar una propuesta, evitando continuar hasta timeout. Herramientas compartidas entre UI y evaluación; presupuesto y opciones de generación limitados | Repetir en el chat gráfico y ampliar a proyectos reales. Los cuatro fixtures no constituyen una certificación general de calidad del modelo. Ver LOCAL-AI.md y benchmarks/agents-real-matrix.json |
| SafeApply | Rust/SQLite, cambios multiarchivo create/edit/delete, baseline/proof, revisión, verificación aislada, apply/revert y recuperación. La generación no escribe el proyecto. Las pruebas requieren recibos reales aprobados | Calificar perfiles de proyectos concretos con servicios y dependencias; no se asume soporte universal |
| Graph | Tree-sitter Python/Go/Rust; imports JS/TS, alias y paquetes; Cargo local, Go modules, servicios Compose e indexación incremental con cambios/borrados/configuración | Semántica de macros/cfg/build tags, imports dinámicos y servicios remotos activos quedan fuera del alcance. El mapa informa truncamiento al llegar a sus límites; no representa entonces el proyecto completo |
| GitHub | Autorización device flow nueva confirmada para Juanexgo, desconexión persistente/reconexión y push/fetch/pull reales repetidos; rama de prueba eliminada. Respuesta 401 real de GitHub con GH_TOKEN de prueba inválido rechazada como credencial no utilizable. PR crear/listar/cerrar aprobado en 0.9. Cancelación/expiración del flujo con procesos controlados | No se envejeció ni revocó el token real del usuario. El 401 de prueba acredita rechazo de credenciales inválidas, no expiración natural observada. Falta el recorrido completo del panel gráfico |
| Distribución | 0.9.1 tiene DMG macOS arm64, NSIS Windows x64 y DEB/AppImage Linux x64 publicados, construidos en el mismo commit y aprobados en sus runners nativos. Feed HTTPS Ed25519 con hash/tamaño, vencimiento, anti-replay/downgrade y descarga manual. 0.9.1 añade configuración y CI de firma que rechazan credenciales ausentes; colector de releases exige checks y paquetes exitosos en el mismo commit | Certificados Developer ID y Windows: el usuario confirmó que no dispone de ellos. No hay firma OS ni notarización aprobadas. El workflow firmado queda preparado, sin ejecutarse con credenciales ficticias. CI 0.9.1 y packaging aprobados en los tres sistemas. Feed público actualizado, estados disponible/al día y descarga completa del DMG con tamaño y SHA256 verificados; nunca se ejecuta automáticamente |
| Calidad | 91 pruebas locales y repetidas con el runtime Electron 44.9.1, sin omisiones; las mismas 91 pruebas, tres de recuperación y dos protocolos reales aprobaron en cada sistema de CI. Linux aprobó además cinco integraciones de Docker/perfiles. Comprobaciones axe de seis paneles iniciales, auditoría NPM sin alertas conocidas y SBOM actualizado | Axe sobre DOM renderizado no comprueba contraste, interacción, estados cargados, lector de pantalla ni shell nativo. SBOM NPM no incluye VSIX, Cargo, binarios o SO |
| Rendimiento | Comparación reproducible de los motores de búsqueda incluidos en ForgeIDE 0.9.1, VS Code 1.142 y Cursor 3.28: 2000 archivos, dos escenarios, nueve muestras por motor, conjuntos de coincidencias idénticos y memoria del proceso medida | No mide inicio/RAM del IDE completo, escritura o respuesta de la UI. Los resultados son similares y no muestran una ventaja clara de ForgeIDE. Falta comparar el editor completo en condiciones controladas |

## Evidencia y límites

- `benchmarks/agents-real-matrix.json`: cuatro roles y recibos reales. El driver hace la revisión y luego verifica/aplica/revierte; no se atribuye aplicación autónoma al agente.
- `benchmarks/agents-real-matrix-before-handoff.json`: conserva el fallo previo de Security después de haber generado una propuesta válida; no se oculta el timeout anterior.
- `benchmarks/search-comparison-macos.json`: muestras, versiones, arquitectura, fixture y alcance del benchmark de búsqueda.
- `docs/LOCAL-AI.md`: configuración reproducible del modelo y comandos de aceptación.
- `docs/SIGNING.md`: credenciales externas necesarias y comprobaciones exigidas.

La CI de 0.9 aprobó macOS/Linux/Windows: https://github.com/Juanexgo/ForgeIDE/actions/runs/44879776714. Sus instaladores siguen en https://github.com/Juanexgo/ForgeIDE/releases/tag/v0.9.0-alpha.1. Esta evidencia histórica no acredita automáticamente los cambios de 0.9.1.

Los agentes Forge no reciben shell ni una herramienta de aplicación directa. Las extensiones Node compatibles sí tienen privilegios del usuario y no constituyen un sandbox. Docker verifica snapshots sin red y con recursos limitados, sin prometer aislamiento absoluto. No se publican credenciales, códigos temporales de autorización ni claves privadas.

## Entrega 0.9.1 publicada

- Release: https://github.com/Juanexgo/ForgeIDE/releases/tag/v0.9.1-alpha.1
- Checks: https://github.com/Juanexgo/ForgeIDE/actions/runs/45280832573
- Packaging: https://github.com/Juanexgo/ForgeIDE/actions/runs/45280860635
- Colector/verificación de instaladores: https://github.com/Juanexgo/ForgeIDE/actions/runs/45282761227

Los cuatro instaladores corresponden a `94972f6a46d6867c5d5361cf8594c2e6e8c816c4`. Los digests públicos de GitHub coinciden con `native-installers.json`. La revisión posterior añade documentación, evidencia de GitHub/feed y el helper de benchmark configurable; no cambia el código de la aplicación empaquetada.

La comparación de búsqueda 0.9.1 dio medianas de aproximadamente 42/38 ms (literal/regex), frente a 44/40 ms en VS Code y 44/46 ms en Cursor. Las muestras se solapan y el entorno no controla la caché del sistema: no se declara una ventaja general a partir de esta diferencia.
