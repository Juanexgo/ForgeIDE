# ForgeIDE frente a VS Code

La meta es que un desarrollador pueda trasladar su trabajo diario a ForgeIDE y medir mejoras en tareas concretas. VS Code sirve como referencia para edición, depuración, extensiones y trabajo remoto. Los resultados de ForgeIDE se registran por versión y plataforma. [Edición](https://code.visualstudio.com/docs/editing/codebasics) · [Depuración](https://code.visualstudio.com/docs/debugtest/debugging) · [Extensiones](https://code.visualstudio.com/api) · [Desarrollo remoto](https://code.visualstudio.com/docs/remote/remote-overview)

| Flujo | ForgeIDE 0.6.0 | Trabajo pendiente |
|---|---|---|
| Explorador y barra | Creación, movimiento exclusivo, recuperación, carpetas vacías, foco y atajos por panel, Problems y contadores. | Papelera nativa, movimientos entre volúmenes, personalización completa y accesibilidad validada. |
| Editar y navegar | Paneles divididos, paleta, símbolos, multicursor, selección rectangular, autoindentación, plegado, snippets integrados con campos y deshacer compartido con operaciones revisadas. | Mapa de teclas configurable, snippets completos, virtualización de archivos grandes y validación de IME. |
| Buscar y revisar | Regex, capturas, búsquedas asíncronas entre varias carpetas, resultados progresivos, búferes abiertos, exclusiones, revisión por fragmentos e historial local. | Inclusiones avanzadas y virtualización de archivos grandes. |
| Lenguajes | Procesos independientes por grupo, perfiles locales, referencias, acciones de código, diagnósticos en el margen, completado junto al cursor e imports revisables. | Instalación y actualización de herramientas, varios documentos por servidor, comandos de acciones y snippets completos. |
| Git y cuentas | Grupos de cambios, diff en dos columnas, preparación por archivos/fragmentos/líneas, commit, ramas, historial seleccionable, worktrees, stash, remotos, fetch/pull/push/sync, clonado y conexión GitHub CLI. | OAuth nuevo/publicación real con cuenta de prueba, grafo, PRs y operaciones avanzadas de historial. |
| Ejecutar y depurar | Terminales con pestañas, selección, copia, colores y tareas. Panel de pruebas por tarea con estados, salida acotada y aviso de cambios. LLDB, breakpoints en el margen, exploración de variables, watches, evaluación y perfiles DAP. | Validar perfiles Node y Python, tareas compuestas, casos individuales de pruebas y depuración en Windows. |
| Personalización | Temas, fuente, zoom, densidad, panel lateral y atajos. Open VSX, VSIX declarativos, versiones fijadas, recuperación y SDK de comandos WASM. | API vscode ejecutable, más contribuciones, firmas, publicación nativa y mapas de teclas editables. |
| Remoto e IA | Se conservan las operaciones locales y la revisión transaccional como base de integración. | SSH, WSL, contenedores e IA con proveedores configurables. |

## Cómo comprobar una mejora

1. **Edición**. Abrir el mismo proyecto, localizar un archivo y una definición, y cambiar el nombre de un símbolo en varios archivos. Registrar tiempo, pulsaciones, memoria y exactitud del resultado.
2. **Revisión**. Aplicar un reemplazo regular, deshacerlo como operación y repetirlo con un archivo modificado tras la vista previa. Verificar los buffers y los archivos guardados.
3. **Recuperación**. Terminar la aplicación durante la edición, restaurar el contenido y verificar que conserva las ediciones recuperables sin reemplazar los cambios hechos en disco fuera del IDE.
4. **Git y depuración**. Ejecutar una sesión con breakpoints, preparar un cambio específico y crear un commit con los archivos correctos.
5. **Experiencia visual**. Repetir los flujos con teclado, escala alta y baja, lector de pantalla e IME. Registrar acceso a las acciones, recortes y errores de foco en macOS, Windows y Linux.

Usa las mismas versiones de servidores, repositorios y hardware al medir. Documenta las funciones que faltan y publica p50/p95 antes de afirmar una ventaja de velocidad. [Protocolo de verificación](VERIFICATION.md)

## Validación pendiente

1. Medir latencia p50/p95, memoria y arranque con proyectos grandes y servidores activos.
2. Validar depuración de Node y Python, Attach y árbol de procesos con adaptadores concretos.
3. Completar pruebas de teclado, IME, lector de pantalla, contraste y escalas en los tres sistemas.
4. Ampliar el SDK, el registro de publicación y los proyectos remotos.
5. Integrar IA opcional con contexto, permisos, resultados de pruebas y revisión recuperable.

## Alcance de esta entrega

El historial en memoria conserva hasta 100 operaciones con un presupuesto de 32 MiB y al menos la operación más reciente. La historia local conserva 20 versiones previas por archivo guardado en el directorio de datos de ForgeIDE.

La entrega 0.6.0 requiere completar pruebas de accesibilidad, IME y distintas escalas. La validación en Windows y Linux y los benchmarks comparativos siguen pendientes.
