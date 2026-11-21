# Estado por sistema — 0.8.1-alpha.1

20 de noviembre de 2026, America/Merida. El objetivo es un IDE profesional. Esta entrega amplía código funcional y pruebas; todavía no completa toda la visión del producto.

| Sistema | Entregado | Qué falta para certificarlo |
|---|---|---|
| Editor | Monaco/Theia, pestañas, splits, multicursor, minimap, búsqueda, preferencias, snippets y mecanismos de refactoring | Pruebas completas de teclado/IME, undo, archivos grandes, accesibilidad y sesiones largas en cada SO |
| Lenguajes | LSP, TypeScript/JS builtin, detector de herramientas y catálogo de extensiones Python/Rust/Go/Java/C++/Swift | Instalar y probar versiones concretas con cada toolchain y depurador. Detectar un ejecutable no acredita que funcione |
| Git | Estado, fragmentos stage/unstage con comprobación de diff vigente, commits, identidad local, ramas/merge, historial/comparación, conflictos revisables, fetch/pull ff-only/push, worktrees, PR list/create | Login real interactivo, expiración/logout y credenciales por SO; push/pull/PR con cuenta y remoto reales; E2E de UI. No se publicaron commits ni PR |
| Cuenta GitHub | Flujo web/device de gh, estado mínimo de cuenta, cancelación, sin exponer tokens ni configurar credenciales globales | Usuario debe completar autorización en navegador. Requiere gh. El proveedor builtin Theia sigue siendo independiente |
| Depuración | DAP y adaptador Node de Theia; breakpoints, variables, stack, watches y stepping de la plataforma | Matriz real Node/Python/Rust/Java/C++ por SO, depuración de pruebas y escenarios remotos |
| Terminal/tareas | PTY Theia, pestañas, perfiles y tareas | Colores, selección, clipboard, atajos y múltiples sesiones comprobados desde UI, no solo smoke |
| Extensiones | Open VSX/VSIX, host VS Code API, 90 builtins | Matriz de instalación/actualización/desinstalación, webviews y contribuciones. No prometer compatibilidad universal ni aislamiento de extensiones Node |
| Forge Agents | Cuatro roles, lectura/grafo/propuestas multiarchivo, petición de tests con confirmación; hard allowlist | Modelos reales configurados, evals, presupuesto de coste, sesiones largas y recuperación. No se hizo ninguna llamada real a modelos |
| SafeApply | Sidecar Rust, journals durables, kernel lock, create/edit/delete hasta 32 archivos, modos Unix, apply/revert, coordinación de buffers de ventanas conectadas, recuperación de proceso y conflictos externos | ACL/Windows, disco lleno/power loss, campañas de fuzzing; no hay bloqueo global contra otros programas. Los buffers se consultan, no se congelan atómicamente |
| Verificación | Node/Python en Docker local, imágenes por digest, red deshabilitada, readonly, usuario sin privilegios y límites; recibos SQLite vinculados a snapshot | Perfiles de dependencias npm/Python implementados en 0.8.1; dos integraciones reales aprobaron preparación, ejecución offline, aplicación/reversión y rechazo por cambios del lockfile. Faltan Yarn/workspaces/registries privados, Rust/Java/C++, coverage y entornos complejos. Se excluyen dotfiles, incluyendo configs; no verificar propuestas sobre ellos |
| Forge Graph | JS/TS, resolución tsconfig/aliases locales/workspace packages, ciclos e impacto, SVG navegable; worker separado con timeout/cola/memoria | Otros lenguajes, indexación incremental/servicios/dinámica y mapa global grande. Máximo 1000 módulos/20000 entradas; informa truncamiento |
| Diagnostics | Métricas de backend, recibos, inventario de lockfiles y scanner npm/OSV con consentimiento, estado obsoleto si cambian inputs | Cobertura/profiling completo, todas las fuentes/registries/lenguajes y auditoría de calidad de resultados. Red no disponible nunca significa libre de vulnerabilidades |
| Infraestructura | SSH/WSL/dev-container de Theia; Docker local usado de verdad en tests | E2E remoto de archivos/LSP/PTY/DAP/permisos. Herramientas propias Forge son locales |
| Experiencia | Shell profesional Theia, tema compartido, paneles Forge, estados de error/carga, formularios y navegación por teclado en grafo | Se comprobó desde UI cuenta GitHub existente, inicialización Git, escritura y guardado. Las pruebas posteriores sufrieron un fallo del servicio visual y bloqueo del Mac. Contraste/lector de pantalla/escalas/sesiones largas pendientes |
| Rendimiento | Worker de grafo acotado y benchmark reproducible 100/1000/10000 archivos | Startup frío/caliente, latencia input-to-paint, RSS de todos los procesos, carga larga y comparación controlada con otros IDE |
| Distribución | Código, lockfiles, sidecar Rust y config macOS/Windows/Linux, CI matrix y preview manual | Ejecutar CI en repo elegido; Windows/Linux reales, firma/notarización, actualizaciones firmadas/rollback y SBOM completo incluyendo plugins/nativos |
| Open source | MIT para código propio, arquitectura/contratos/tests/benchmarks/workflows locales | Publicación en repositorio elegido, evidencias públicas y revisión de avisos de terceros. No existe remoto autorizado en esta tarea |

## Evidencia

0.8.1: 54 pruebas locales y cinco integraciones reales de Docker aprobadas sin omitidos (tres recorridos base y dos perfiles de dependencias). La evidencia previa de 0.8 conserva 3 escenarios de cierre forzado y 3 integraciones Docker reales; no se cuentan como nuevas ejecuciones si no se repitieron. Los tests Git usan repositorios temporales reales y un doble de gh para probar manejo seguro, no una cuenta OAuth real. La compilación completa browser/node/electron pasó. Los resultados de paquete se registran en VERIFICATION.md cuando se comprueban.

La auditoría del lockfile informa 0 críticas, 0 altas, 37 moderadas y 4 bajas (rutas repetidas; cuatro advisories únicos). Se corrigieron las altas observadas, pero no se afirma ausencia de vulnerabilidades. Los saltos mayores forzados de dependencias se comprobaron en build y tests locales, no en todos los plugins/OS.

## Siguiente puerta de producto

Una beta exige probar desde el instalador los flujos de edición, Git/login, terminal, DAP, plugins y recuperación; ejecutar la matriz de plataformas; resolver/justificar findings; validar un proveedor de IA con evals y proyectos reales; y preparar firma/actualizaciones. El código implementado no convierte automáticamente esas verificaciones pendientes en aprobadas.

## Cambios respecto a la lista 0.7.1 del usuario

- Cambios multiarchivo, creación/borrado, alias JS/TS y servicio Rust activo ya estaban implementados y probados en 0.8.
- 0.8.1 añade preparación de dependencias públicas fijadas Node/Python, plan con consentimiento, imagen por ID inmutable, rechazo si cambian manifiestos, recibo ligado a dependencias y cancelación más rápida del cliente.
- El paquete final pasó 54 tests también bajo Electron, PTY real, motor Rust y worker; llegó a ready con 90 plugins e integridad del DMG aprobada. La interfaz reconoció la cuenta gh existente. Eso no prueba onboarding OAuth de una cuenta nueva, expiración/logout, push o creación de PR reales.
- Se conserva la base Theia para editor/LSP/DAP/extensiones y los workflows locales para Windows/Linux. Ningún runner remoto ni firma se ha ejecutado aquí.
- No se modificó el motor Rust en 0.8.1; mantiene versión 0.8 y protocolo 1.
- No se puede cerrar toda la visión con las comprobaciones actuales. Firma/actualización requieren infraestructura de release elegida, y la matriz multiplataforma exige esos sistemas. Comparación con otros IDE, accesibilidad, semántica de otros lenguajes y remoto siguen siendo trabajo de producto.
