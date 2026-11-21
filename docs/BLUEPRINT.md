# Implementación de referencia actual: 0.8.1

La arquitectura aprobada sigue siendo Theia/Electron + React/TypeScript; Monaco proporciona edición, LSP/DAP y Open VSX integran herramientas. El sidecar Rust es ahora real (engine/, protocolo 1), con build distribuido y pruebas de recuperación. Node coordina Git/gh, verificación Docker, SQLite y scanners. El grafo JS/TS corre en worker separado; no representa semántica universal de todos los lenguajes.

0.8.1 añade perfiles de dependencias independientes del código: plan por digest → consentimiento → contexto mínimo de build → imagen inmutable → verificación offline → revisión/aplicación/reversión. Los detalles vigentes están en SECURITY-AND-API.md, DEPENDENCY-PROFILES.md, ESTADO-COMPLETO.md y VERIFICATION.md. Las fases de abajo sirven como PRD y roadmap; un requisito previsto no debe confundirse con una función comprobada.

# ForgeIDE: producto y arquitectura profesional

## PRD

**Problema:** desarrollar exige conectar editor, contexto del proyecto, herramientas y asistentes sin perder control sobre los cambios. ForgeIDE debe reducir esa fricción y conservar evidencia de qué se cambió, por qué y con qué resultados.

**Usuarios:** desarrolladores individuales y equipos que trabajan con repositorios locales o remotos en TypeScript/JavaScript, Python y Rust inicialmente. Java, C++, Go y Swift se incorporan mediante perfiles de lenguaje comprobados. Reclutadores y contribuidores deben poder reproducir la compilación y los benchmarks.

**Principios:** recuperación primero; IA opcional; estado explícito; teclado y accesibilidad; compatibilidad comprobada; ninguna promesa de rendimiento sin medición.

**Versión profesional mínima:** edición con multicursor, columnas, plegado, snippets, autoindentación y undo; varios LSP, refactorizaciones y diagnósticos; terminales y tareas; DAP con variables/watches; Git con staging por fragmentos, conflictos y ramas; extensiones actualizables; instalación y actualización confiables. Adoptar Theia reduce trabajo de plataforma, pero cada flujo debe probarse en el producto ensamblado.

**Fuera de la alpha:** VS Code Marketplace universal, agentes autónomos sin revisión, aislamiento de extensiones Node, superioridad demostrada, instaladores firmados en tres sistemas.

## Diferenciación medible

| Referencia | Base que ForgeIDE debe igualar | Apuesta propia | Evidencia exigida |
|---|---|---|---|
| VS Code | Extensiones, Git, LSP, terminal, DAP y personalización | Revisión recuperable para cambios manuales y asistidos | Misma tarea, mismos repositorios, cero pérdida de datos |
| Cursor | Revisión de cambios y flujos de agentes | Evidencia de verificación vinculada al parche exacto y permisos visibles | Hash del parche y entorno, logs y pruebas ejecutadas |
| Zed | Respuesta al escribir y uso eficiente del equipo | Presupuestos de rendimiento de toda la sesión, incluidos plugins | Latencia p50/p95/p99 y RSS por proceso con metodología pública |

Estas son metas de producto. No se ha realizado aquí un benchmark comparativo.

## Decisiones y compromisos

| Capa | Decisión | Motivo y coste |
|---|---|---|
| Plataforma | Theia 1.77.0 | IDE modular y host compatible con muchas API de VS Code; exige validar las diferencias de API |
| Desktop | Electron 44.9.1 | Integración oficial de Theia y comportamiento consistente; mayor descarga y memoria que la app Rust |
| UI | TypeScript, React, widgets Theia | Reutiliza shell, docking, accesibilidad y temas; evitar crear un segundo sistema de diseño |
| Buffer / edición | Monaco de Theia | Modelos, selecciones, undo y edición madura; no mantener dos motores de texto simultáneamente |
| Parseo | Servicios de lenguaje + AST TS inicial; Tree-sitter opcional en sidecar | LSP gobierna semántica; Tree-sitter ayuda en lenguajes sin LSP, con versiones de gramáticas y presupuestos |
| Renderer | Renderer de Monaco/Electron | No se reutiliza el renderer egui; GPU/UI nativos propios quedan archivados |
| LSP | Extensiones/perfiles Theia | Un servidor por lenguaje/raíz según configuración; UTF-16 y versiones de documentos se delegan al editor |
| Debug | DAP Theia + adaptadores probados | Node builtin; Python/Rust requieren extensiones/adaptadores y toolchains verificados |
| Terminal | Theia + PTY nativa | Preparar módulos para cada ABI de Electron y plataforma |
| Git | SCM + builtin Git | Reutilizar gestor de credenciales y git instalado; PRs y OAuth requieren pruebas aparte |
| Extensiones | Plugin host y Open VSX | Declarar compatibilidad por ID/versión; no equiparar separación de proceso con sandbox |
| IA | Theia AI, proveedores intercambiables | Sin acoplar a un proveedor; contextos visibles, permisos y costos; proveedores sin configurar = sin llamadas |
| Rust | Servicios selectivos en proceso separado, siguiente etapa | Reutilizar componentes legados solo tras medir beneficio; protocolo versionado, límites y cancelación |
| Datos | JSON durable para propuestas + SQLite para recibos | SQLite nativo persiste recibos versionados; journal conserva propuestas. La transacción multiarchivo sigue pendiente |

## Estructura

```text
apps/desktop/             composición Theia, lanzador Electron y distribución
packages/workbench/src/
  browser/               panel, herramienta AI y contribuciones
  common/                contratos RPC versionables
  node/                  historial, aplicación controlada y grafo estático
tests/                   pruebas de conflictos, recuperación y grafo
docs/                    PRD, contratos, seguridad, validación y prompt
legacy/native-0.6/       referencia preservada, no runtime activo
.github/workflows/       comprobaciones multiplataforma
plugins/                 descargados; se regeneran desde versión fijada
```

## Flujos y criterios de aceptación

### P0 — migración instalable

1. Instalar dependencias con lockfile, compilar, preparar ABI y empaquetar.
2. Desde `.app` fuera del árbol de desarrollo abrir una carpeta sin Node instalado, editar y recuperar sesión; terminal PTY; buscar; Git status; instalar extensión de prueba; Node breakpoint.
3. Inicio de sesión GitHub en navegador, cancelación, expiración, cierre de sesión y keychain. No registrar tokens. No confundir autenticación con `user.name`/`user.email`.
4. Registro de resultados por SO, arquitectura, versión y extensión. No promover a beta con fallos de pérdida de datos.

Estado: compilación y arranque de desarrollo comprobados; matriz integral pendiente.

### P1 — calidad diaria y apariencia

- Misma densidad, márgenes, estados de carga/error y acciones de teclado en todos los paneles.
- Probar multicursor, columna, IME, teclado internacional, selección RTL, archivos Unicode/CRLF y undo tras format/rename.
- LSP TS/Python/Rust simultáneos, rename multiarchivo, code actions/imports y diagnósticos con documentos sin guardar.
- Git: stage/unstage por hunk, rama/worktree, comparación de commits, merge de tres vías, conflictos con binarios, repositorio sin remoto y credenciales canceladas.
- DAP: breakpoints de margen, expandir variables, stack, watches, evaluación, detach y depuración de tests.
- Terminal: varias sesiones, colores/Unicode, resize, copiar/pegar, procesos largos y cancelación de tareas.
- Workspaces multiraíz, exclusiones y archivos grandes; búsqueda cancelable durante indexación.
- Aceptación: jornada reproducible de ocho horas por SO sin cierre inesperado ni pérdida silenciosa; auditoría de teclado, lector de pantalla y escalas 100/150/200%.

### P2 — SafeApply profesional

- Cambios multiarchivo con snapshots, revisión por fragmentos, precondición de versión de buffer y journal transaccional. Creaciones/borrados explícitos.
- Escrituras coordinadas con el modelo del editor: no modificar disk mientras existe un buffer dirty en ninguna ventana; undo lógico completo.
- Recuperación ante terminación en cada paso, almacenamiento lleno, permisos, ACL, concurrent writers y enlaces.
- Runner aislado por OS/contenedor: snapshot mínimo, exclusión de secretos, sin red por defecto, límites de CPU/RAM/tiempo, filesystem acotado y sin socket Docker del host.
- Recibo de pruebas: hash del parche, snapshot y comandos exactos, exit code, duración, entorno y log acotado. Cualquier cambio invalida el recibo.
- Aceptación: una propuesta no puede inventar pruebas aprobadas; revertir conserva cambios posteriores o informa conflicto sin sobrescribirlos.

Estado alpha: reemplazo de archivo existente, comparación, journal y conflictos; hay runner Node aislado con recibos y checks de snapshot; falta transacción multiarchivo.

### P3 — Agents / Graph / Diagnostics

- Agentes implementar/pruebas/revisión con permisos por herramienta. Editor/review-only como valor inicial. Shell y red requieren consentimiento específico en el producto.
- Historial durable, reanudación, límite de tokens/costo/tiempo, cancelación y contexto visible. Texto de archivos y outputs se considera contenido no confiable.
- Graph incremental con IDs estables, imports/exports, tipos, ciclos y análisis de impacto; distinguir certeza semántica de heurística. Añadir otros lenguajes después de fixtures.
- Diagnostics: tareas comprobadas de tests, dependencia/auditoría, recursos y rendimiento. Resultados con fecha y alcance, jamás un semáforo verde si el análisis no corrió.
- Aceptación: benchmark de tareas con tests ocultos y evaluación de cambios incorrectos; tasa de éxito y regresiones publicada junto a metodología.

### P4 — distribución y remoto

- SSH/WSL/dev containers: backend, extensiones, paths, LSP, PTY y DAP en el host correcto. Sidecars propios en remoto solo con protocolo y despliegue comprobados.
- macOS universal o dos arquitecturas, Windows x64/arm64 cuando se valide, Linux x64 con matriz de distribuciones mínima.
- Firma, notarización, SBOM, avisos legales, checksums, actualizaciones firmadas por canal y rollback.
- Aceptación: instalación limpia, actualización N-1→N, cancelación/offline, verificación de firma y recuperación de actualización interrumpida.

## Presupuestos iniciales (objetivos, no resultados)

Hardware de referencia: 8 núcleos, 16 GB RAM, SSD; anotar modelo y SO. Repositorios fijados por commit de 1k/10k/100k archivos. Ejecutar diez corridas frías/calientes y reportar distribución, no solo media.

- Editor ready p95 ≤ 3 s en workspace pequeño; input-to-paint p95 ≤ 16 ms y p99 ≤ 50 ms bajo carga.
- Búsqueda primer resultado p95 ≤ 300 ms para fixture 10k; cancelación ≤ 100 ms.
- Sin aumentos monotónicos de memoria tras 100 ciclos abrir/cerrar. Separar RSS de backend, renderer, plugin host, LSP y sidecars.
- Fichero de 20 MiB abre en modo limitado sin bloquear; no prometer todas las funciones semánticas para archivos enormes.
- Regresión >10% frente al baseline exige investigación antes de release. La base Electron puede no ganar en RAM al cliente Rust; medir antes de afirmar ventajas.

## Fuentes de la decisión

- https://theia-ide.org/docs/composing_applications/
- https://theia-ide.org/docs/architecture/
- https://theia-ide.org/docs/authoring_vscode_extensions/
- https://theia-ide.org/docs/theia_ai/
- https://github.com/eclipse-theia/theia-ide
