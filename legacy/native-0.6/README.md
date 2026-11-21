# ForgeIDE 0.6.0

ForgeIDE es un IDE nativo de escritorio escrito en Rust. Esta compilación de desarrollo incluye edición local con recuperación, terminal, Git, herramientas LSP, depuración DAP y extensiones locales WebAssembly.

## Novedades de 0.6.0

- Explorador con creación, renombrado/movimiento, carpetas vacías, copia de rutas y recuperación de elementos retirados.
- Barra lateral desplegable, atajos por panel, foco de búsqueda protegido y contadores útiles.
- Ventana Problems, detección de herramientas de lenguaje y corrección del arranque mediante shims.
- Depuración con teclas de función y navegación al frame detenido.
- Detección de tareas de prueba, compilación y desarrollo en proyectos compatibles.

[Guía de la barra y explorador](docs/WORKBENCH.md). Esta versión mantiene las funciones de Git/GitHub 0.5.0 y extensiones 0.4.0. Continúa siendo una compilación de desarrollo, con diferencias documentadas frente a VS Code.

## Novedades de 0.5.0

- Source Control con grupos, preparación por archivo/fragmento/líneas y diff en dos columnas.
- GitHub accounts con autorización en navegador, comprobación de cuenta, cambio de cuenta y salida.
- Inicialización, clonado, autor local, remotos, fetch/pull/push/sync y publicación revisada.
- Ramas, historial seleccionable, worktrees, stashes y descarte con copia en el historial local.
- Operaciones cancelables, comprobación de HEAD/rama y destinos de publicación visibles.

[Guía de Git y GitHub](docs/GIT.md). La autorización y publicación reales en GitHub requieren comprobación con una cuenta de prueba. No se declara paridad completa con las funciones Git de VS Code.

## Novedades de 0.4.0

- Catálogo Open VSX con búsqueda, ficha, versiones, inspección e instalación.
- Importación VSIX de temas y snippets, con avisos de compatibilidad.
- Versiones fijadas, comprobación de actualizaciones, actualización en cola y recuperación de la versión anterior.
- Activación global o por carpeta y restauración de extensiones desinstaladas.
- Perfiles portátiles y paquetes nativos .forge-extension con módulos incorporados.
- Comandos nativos en la paleta, dependencias comprobadas e inspector/empaquetador de línea de comandos.

Consulta [EXTENSIONS.md](docs/EXTENSIONS.md) para el flujo y el SDK. No se ejecuta la API JavaScript de VS Code; los VSIX exclusivamente ejecutables se rechazan. La [lista de aceptación](docs/EXTENSION-ACCEPTANCE.md) distingue lo implementado de lo pendiente.

## Novedades de 0.3.0

- Espacios con hasta 16 carpetas. Usa Explorer → menú → Add Folder to Workspace. Cada carpeta conserva sus paneles de herramientas. La sesión guarda sus carpetas y pestañas.
- Búsqueda y reemplazo entre carpetas con exclusiones por carpeta. La búsqueda puede empezar durante la indexación e indica los resultados parciales.
- Gestor de extensiones locales con instalación, actualización, activación, desactivación y desinstalación. Los paquetes aportan snippets, temas y transformaciones WebAssembly.
- Panel Testing para tareas de prueba, ejecución individual o secuencial, salida acotada, estado y aviso cuando cambian los documentos abiertos.
- Las revisiones permiten aplicar los fragmentos seleccionados, guardar esos archivos y ejecutar las pruebas cargadas de la carpeta activa.
- Git permite preparar líneas seleccionadas y comparar dos hashes de commit.
- El título de la ventana muestra la versión.

[Guía de extensiones](docs/EXTENSIONS.md) · [Perfiles de proyecto y pruebas](docs/PROJECT-PROFILES.md)

## Corrección 0.2.1

La terminal reserva espacio para su mensaje de estado y mantiene estable la altura del panel. Una prueba de 40 ciclos de interfaz comprueba que el editor conserva espacio visible.

## Novedades de 0.2.0

- Plegado de bloques desde el margen en Rust, TypeScript y TSX, conservando las posiciones del texto original.
- Sugerencias LSP junto al cursor con flechas, Enter y Tab. Los imports adicionales pasan por revisión.
- Snippets integrados con campos mediante Tab y Shift+Tab. En Rust usa fn, test o if seguido de Tab. En TypeScript/JavaScript usa fn, log o if. En Python usa def o if.
- Historial de deshacer para escritura, multicursor y operaciones revisadas. Revisión con selección de fragmentos e historial local por archivo.
- Servidores independientes por lenguaje, referencias, acciones de código y diagnósticos en el margen según las capacidades del servidor.
- Git con preparación por fragmentos, ramas, historial, comparación con el padre y worktrees. Vista de conflictos y revisión de su resolución.
- Breakpoints desde el margen, exploración de variables, watches y evaluación. Perfiles DAP configurables.
- Varias terminales con selección, copia, colores ANSI de 16/256 colores y RGB, y tareas de proyecto.
- Temas claro y oscuro, tipografía, zoom y densidad desde **Settings**, con preferencias persistentes.

El plegado y los snippets cubren un conjunto inicial de lenguajes y operaciones. Los snippets LSP se expanden con valores iniciales. Las variables, transformaciones y campos anidados de snippets siguen pendientes. Los archivos conservan el límite de 10 MiB y el trazado completo de texto de egui.

## Ejecutar

Instala Rust mediante rustup y ejecuta estos comandos desde la raíz del proyecto.

```sh
cargo run --locked -p forge-desktop
cargo run --locked -p forge-desktop -- /ruta/al/proyecto /ruta/al/proyecto/main.rs
```

En Linux hacen falta las bibliotecas del sistema para OpenGL y el teclado. En Ubuntu instala `libgl1-mesa-dev` y `libxkbcommon-dev`. El selector de archivos también necesita un portal de escritorio operativo. Los paquetes de desarrollo corresponden a macOS Apple Silicon, Ubuntu ARM64 y Windows x64. El paquete de macOS usa firma ad hoc.

## Usar el IDE

| Acción | Cómo |
|---|---|
| Editar archivos | **New File**, **Open File** u **Open Folder**. Las pestañas conservan cambios mientras alternas entre archivos. Al iniciar sin rutas desde la línea de comandos, ForgeIDE restaura las pestañas de la sesión anterior. Usa Cmd/Ctrl+S para guardar, Cmd/Ctrl+W para cerrar, Cmd/Ctrl+F para buscar y Cmd/Ctrl+G para avanzar. |
| Editar en varios puntos | En el editor, presiona Cmd/Ctrl+D para seleccionar una palabra o el texto marcado; repite para añadir coincidencias. Para una selección rectangular, marca dos extremos en líneas distintas y pulsa Cmd/Ctrl+Alt+D. Escribe, pega o borra para editar todos los puntos. Esc termina el grupo; Cmd/Ctrl+Z revierte el grupo si todavía es la última edición. |
| Mantener la sangría | Enter alinea la línea nueva con la actual y agrega un nivel si el texto anterior termina en una llave, un paréntesis o un corchete de apertura. |
| Evitar sobrescrituras | El editor avisa cuando un archivo abierto cambia en disco. Puedes recargar un archivo limpio. Si hay una colisión al guardar, ForgeIDE conserva las ediciones abiertas. **Save Copy** crea un archivo nuevo y se niega a reemplazar uno existente. El editor conserva BOM y contenido UTF-8, incluidos los saltos de línea existentes. |
| Recuperar cambios | **Restore Edits** abre el selector de registros locales. Los registros guardan la versión base y los cambios. Si el original cambió o falta, usa **Save Copy**. Los registros previos a una recuperación permanecen disponibles hasta que se eliminen desde el gestor de archivos. |
| Editar en dos paneles | Usa el icono **Toggle Split Editor**, Cmd/Ctrl+barra invertida o **Open to Side** desde el menú de un archivo en Explorer. Arrastra el separador para cambiar el espacio de cada editor. Guardar y buscar actúan sobre el panel con foco. |
| Usar la paleta | Cmd/Ctrl+P abre archivos y Cmd/Ctrl+Shift+P muestra comandos. El selector **Symbols** navega por declaraciones del búfer activo en Rust y TypeScript. |
| Reemplazar y revisar | **Find** ofrece búsqueda literal, regex y opción de ignorar mayúsculas. **Review Replace All** muestra antes y después. En la paleta puedes deshacer o rehacer la última edición revisada. |
| Buscar y reemplazar en el proyecto | En **Search**, escribe un término, activa **Regex** o **Ignore case** y acota las rutas con **Filter paths**. Activa **Replace** para revisar el resultado en varios archivos. La búsqueda toma en cuenta los cambios abiertos. **Apply to Editor** actualiza los búferes y **Save All** guarda los archivos sin conflicto. |
| Navegar y buscar | Expande carpetas en **Explorer**, abre archivos con Cmd/Ctrl+P o usa **Search** para localizar texto guardado y saltar a un resultado. Se omiten rutas grandes o ilegibles, y se informa si se alcanza un límite. |
| Trabajar con Git | Abre la raíz del repositorio, selecciona **Git** y activa Git para esa carpeta. Actualiza el estado, revisa diffs, prepara archivos y crea un commit con los cambios preparados. Git puede ejecutar filtros y hooks configurados al realizar operaciones explícitas. |
| Ejecutar comandos | Selecciona **Terminal → Start Shell**. Haz clic en la terminal para escribir. Shift+Esc devuelve el foco al IDE. |
| Usar herramientas de lenguaje | Selecciona **Language Server**, elige el ejecutable instalado e inicia la sesión. Puedes consultar símbolos, ir a una definición en el proyecto y mostrar completados. Las propuestas de completado, incluidos los imports adicionales que proporciona el servidor, renombrado y formato pasan por una vista previa. Guarda los búferes resultantes cuando estés conforme. |
| Depurar programas | Abre un archivo fuente del proyecto y selecciona **Debugger**. Elige `lldb-dap` y un binario con símbolos para **Launch**, o un PID local para **Attach**. Introduce las líneas de interrupción y consulta pila y variables al detenerse. |
| Ejecutar extensiones locales | Selecciona **Extensions**, elige un archivo WebAssembly y revisa la transformación antes de aplicar. El módulo recibe el texto del búfer activo y se ejecuta sin funciones anfitrionas. |

El resaltado Tree-sitter está habilitado para Rust, TypeScript y TSX hasta 128 KiB. El límite de edición es de 10 MiB para archivos UTF-8. La búsqueda de carpeta admite texto literal y expresiones regulares. Revisa archivos de hasta 512 KiB y muestra hasta 200 coincidencias de búsqueda. La revisión de reemplazos tiene un límite de 500 cambios en 64 archivos, con un aviso si se supera. Las herramientas externas se inician desde los paneles y usan los permisos de la sesión del usuario.

## Verificar

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Las pruebas que requieren herramientas instaladas se ejecutan por separado. [Instrucciones de verificación](docs/VERIFICATION.md)

## Arquitectura y estado

La versión 0.1.1 incorpora un rediseño de la ventana nativa con herramientas en la barra lateral, pestañas con cierre individual, ruta del archivo, numeración de líneas y posición del cursor. Usa Inter para la interfaz e IBM Plex Mono para el código, ambas bajo SIL OFL 1.1. El editor usa egui/eframe con diálogos nativos. Esta elección permite ejecutar y depurar el flujo completo desde temprano. La selección definitiva del toolkit depende de medir latencia, IME y accesibilidad en el conjunto de plataformas objetivo.

[API de extensiones](docs/EXTENSIONS.md) · [Empaquetado](docs/PACKAGING.md)

El [blueprint](docs/BLUEPRINT.md) conserva los requisitos de la versión 1.0. La [verificación](docs/VERIFICATION.md) registra el alcance probado y los hitos pendientes. El [prompt maestro para Codex](docs/CODEX-PROMPT.md) guía el trabajo restante.

[Paridad funcional con VS Code y método de comparación](docs/PARIDAD.md)

## Perfiles de proyecto

La [guía de configuración](docs/PROJECT-PROFILES.md) explica las tareas, exclusiones, servidores y adaptadores. Las tareas y los adaptadores se ejecutan al accionar sus controles.

[PARIDAD.md](docs/PARIDAD.md) recoge el estado de cada área y [VERIFICATION.md](docs/VERIFICATION.md) distingue pruebas actuales e históricas.
