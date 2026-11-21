# Verificación de ForgeIDE

## Entrega 0.6.0

En macOS Apple Silicon pasaron 93 pruebas automatizadas (87 del escritorio, dos del ejecutable/SDK y cuatro del motor de texto), formato y Clippy con advertencias tratadas como errores.

Se ejecutaron explícitamente y pasaron cinco integraciones: Rust Analyzer, TypeScript Language Server, ambos servidores simultáneos, LLDB de Xcode y búsqueda/descarga/verificación/importación de Gruvbox desde Open VSX. Se utilizaron fixtures separados de los proyectos del usuario.

Las pruebas nuevas cubren rutas y movimientos exclusivos del sistema, recuperación y colisiones al restaurar, carpetas vacías, renombrado de búferes con cambios sin guardar, búsqueda mediante atajo sin modificar el documento, breakpoints con Unicode, detección de tareas, prevalencia de configuración explícita y conservación del nombre de ejecutables con shims.

La compilación release y la firma ad hoc de macOS pasaron, y hdiutil verificó la integridad del DMG. La revisión visual final de 0.6.0 quedó bloqueada porque la Mac estaba bloqueada. Las pruebas de interfaz con eventos simulados pasaron; no sustituyen la revisión interactiva de todos los controles.

La revisión interactiva anterior de 0.5.0 sí comprobó el panel Git y una cuenta GitHub existente, activa y almacenada en el llavero. No se hizo un nuevo consentimiento OAuth ni una publicación real en GitHub. Windows/Linux, IME, lector de pantalla, perfiles Node/Python y la supervisión completa de procesos requieren validación adicional.


## Entrega 0.5.0

En macOS Apple Silicon pasaron 83 pruebas automatizadas (77 del escritorio, dos del ejecutable/SDK y cuatro del motor de texto), formato y Clippy con advertencias como errores.

Se comprobaron con repositorios temporales reales: clonado, autor local, publicación de ramas en un remoto bare local, fetch/pull/sync, rechazo de divergencia, HEAD y rama desactualizados, índice sin primer commit, ramas, historial seleccionable, stashes con archivos nuevos, descarte con copia previa, destinos de push diferentes y cancelación del proceso.

Se corrigió una interacción de --literal-pathspecs con los comandos internos de stash que conservaba los archivos sin seguimiento después de guardarlos. La prueba verifica ahora tanto su retirada como su restauración manteniendo el stash.

La firma ad hoc y la integridad del DMG pasaron. En la app empaquetada ForgeIDE 0.5.0 se revisaron el panel Source Control, la rama, el contador de archivos, los grupos y el área de commit. La comprobación de cuentas detectó una cuenta GitHub CLI existente y activa, con credenciales en el llavero del sistema. No se prepararon, descartaron ni enviaron los cambios del proyecto del usuario durante esa revisión.

El consentimiento de un acceso OAuth nuevo y la creación/publicación en GitHub no se ejecutaron con una cuenta de prueba. La aplicación ofrece esos controles mediante GitHub CLI; se requiere validarlos antes de declararlos estables. Las pruebas de transporte usaron remotos locales. Windows/Linux y la supervisión completa de procesos externos siguen pendientes de validación nativa.


## Entrega 0.4.0

En macOS Apple Silicon pasaron 70 pruebas automatizadas: 64 del escritorio, dos del ejecutable/SDK y cuatro del motor de texto. Pasaron también cargo fmt y Clippy con advertencias tratadas como errores.

Una prueba explícita con internet consultó Open VSX, descargó jdinhlife.gruvbox@1.29.2, verificó su SHA-256 e importó seis temas. En la ventana de la aplicación empaquetada 0.4.0 se comprobó el catálogo, la consulta por ID y versión, la revisión de contribuciones y la instalación; la ficha instalada mostró sus seis temas y controles de activación. No se comprobó visualmente el ciclo completo de actualización/rollback/perfiles: estos flujos tienen cobertura automatizada.

Las pruebas nuevas cubren VSIX declarativos y mixtos, omisiones explícitas, archivos y rutas inseguros, enlaces simbólicos, expansión excesiva, JSON con comentarios, includes cíclicos, perfiles, cancelación, comandos empaquetados, estado por carpeta, fijación de versiones, rollback, recuperación después de desinstalar, dependencias transitivas y ciclos, escrituras desde ventanas desactualizadas y conservación de la copia anterior tras una actualización fallida.

Se verificaron la firma ad hoc, el arranque de la app empaquetada y la integridad del DMG. El inspector y el empaquetador del ejecutable release generaron e inspeccionaron el ejemplo portable Forge Toolbox.

No se ejecutaron de nuevo las cuatro integraciones LSP/DAP de 0.3.0; siguen disponibles como pruebas explícitas. Windows, Linux, accesibilidad, IME y uso prolongado no se validaron de nuevo para 0.4.0. La matriz CI está configurada para esos tres sistemas. Esta entrega no implementa un host JavaScript compatible con toda la API vscode.


## Entrega 0.3.0

En macOS Apple Silicon pasaron 52 pruebas automatizadas, formato y Clippy. Además pasaron cuatro pruebas con herramientas reales, incluyendo Rust Analyzer y TypeScript activos a la vez, referencias, renombrado, completado y LLDB con evaluación de un watch.

Las comprobaciones nuevas cubren dos carpetas con un archivo externo excluido, reemplazos basados en búferes sin guardar, conservación de herramientas por carpeta, persistencia de raíces, búsqueda parcial durante la indexación, actualizaciones atómicas de extensiones, activación de contribuciones, temas, rutas de transformaciones, worker con módulos incorporados al paquete y tareas de prueba con salida extensa y fallos.

Las pruebas Git existentes se ampliaron para preparar solo una línea de un fragmento y comparar dos commits reales. El ejecutor de pruebas también comprueba que cambie la firma de documentos al editar y guardar.

La revisión visual de 0.3.0 quedó bloqueada por la sesión de macOS bloqueada. Windows, Linux, lector de pantalla, IME, jornadas largas y benchmarks frente a otros IDE requieren validación adicional.

## Prueba interactiva y corrección 0.2.1

Pasaron las 46 pruebas automatizadas, formato y Clippy. El instalador DMG pasó su comprobación de integridad.

En 0.2.0 se comprobó en la ventana el plegado, la expansión de un snippet, la navegación de sus campos, el guardado y deshacer/rehacer. La terminal ejecutó echo FORGE_OK y abrió una segunda pestaña. Se detectó que su panel aumentaba de altura en cada ciclo al añadir el mensaje de estado.

Una prueba de regresión reprodujo la reducción del espacio del editor desde 520 puntos hasta cero. La corrección reserva el espacio de la línea de estado dentro del panel. La Mac se bloqueó antes de completar otra revisión visual.

## Entrega 0.2.0

Pasaron 45 pruebas automatizadas y 4 pruebas con herramientas reales en macOS Apple Silicon. Formato y Clippy también pasaron. Las cuatro integraciones se ejecutaron de forma explícita además de la suite habitual.

Se verifican la navegación de campos de snippets con eventos de teclado, el plegado conservando texto y posiciones, la cadena de deshacer entre escritura y edición revisada, la selección de un fragmento entre apariciones idénticas, la retención de una versión anterior al guardar, las exclusiones y la preparación de un único fragmento Git en un repositorio real.

Las integraciones reales comprueban referencias, renombrado y completado en Rust Analyzer y TypeScript Language Server. Una prueba adicional mantiene ambos servidores activos y solicita información a cada uno. LLDB comprueba un breakpoint, la variable answer y su evaluación como watch.

Las pruebas de la interfaz usan egui con eventos simulados y representación de texto. La revisión visual de la ventana de esta entrega quedó impedida por la pantalla bloqueada. Los perfiles de adaptadores Python y Node están configurables y requieren validación con esos adaptadores. La matriz histórica de abajo corresponde a versiones anteriores.


En 0.1.4 se probaron en macOS las búsquedas de varias coincidencias por línea, los patrones de varias líneas, los reemplazos con capturas, el filtro de rutas y la lectura de búferes abiertos con cambios sin guardar. La prueba de **Save All** comprueba que un archivo modificado por otro proceso se conserva mientras otro archivo sin conflicto se guarda.

En 0.1.3 pasaron 29 pruebas automatizadas en macOS. Se probó escribir en el panel derecho y guardar mediante el documento que quedó activo, sin modificar el archivo izquierdo. También se comprobaron el cierre de pestañas con dos paneles y la persistencia de la distribución.

En 0.1.2 se verificó en macOS el arranque de la ventana con la paleta, el archivo abierto y su minimapa. Pasaron 27 pruebas automatizadas, formato y Clippy. Las pruebas nuevas cubren la clasificación de archivos, los símbolos locales, la navegación regex, las capturas y deshacer/rehacer sin tocar lo guardado. La prueba interactiva al accionar la paleta se interrumpió por un cambio de foco del sistema.

ForgeIDE 0.1.1 rediseña la ventana nativa. En macOS se revisó visualmente una compilación del rediseño con un proyecto abierto y se corrigió la alineación del árbol. La compilación final pasó 22 pruebas automatizadas, formato y Clippy. La matriz de abajo también recoge las pruebas y paquetes de la versión 0.1.0.

## Pruebas reproducibles

| Plataforma | Verificación observada |
|---|---|
| macOS Apple Silicon | 21 pruebas automatizadas y pruebas con Rust Analyzer, TypeScript Language Server y LLDB de Xcode. Se editó, guardó y reabrió un archivo en la ventana. Se verificaron el DMG, la firma ad hoc y el arranque del paquete. |
| Ubuntu 24.04 ARM64 | 21 pruebas automatizadas, Clippy, instalación del DEB, arranque en X11 virtual, desinstalación y prueba real de LLDB 18. |
| Windows x64 | Compilación cruzada exitosa para `x86_64-pc-windows-gnu` y MSI comprobado con el listado de sus archivos. Falta ejecutar el instalador y la aplicación en Windows. |


Desarrollado en macOS Apple Silicon. La matriz GitHub Actions está preparada para compilar y probar en macOS, Windows y Ubuntu, y genera archivos de desarrollo cuando se inicia manualmente.

Las pruebas automatizadas cubren edición transaccional Unicode, coordenadas UTF-8/16/32, guardado y recuperación, búsqueda, perfiles Tree-sitter, framing de protocolos, selección de breakpoints, terminal POSIX y el flujo Git con un repositorio temporal.

### LSP con Rust Analyzer

Instala el componente de Rust Analyzer con rustup y ejecuta desde la raíz del proyecto.

```sh
FORGE_TEST_LSP="$(rustup which rust-analyzer)" \
FORGE_TEST_LSP_ROOT="$PWD/fixtures/rust-analyzer" \
cargo test --locked -p forge-desktop talks_to_real_rust_analyzer -- --ignored
```

La prueba inicia un proceso real, solicita la firma de un símbolo, previsualiza su renombrado y comprueba un completado sobre un búfer modificado.

### TypeScript Language Server

Con Node.js instalado, usa un directorio de herramientas separado.

```sh
tools_dir="$(mktemp -d)"
npm install --prefix "$tools_dir" --ignore-scripts typescript@5.8.3 typescript-language-server@4.3.4
FORGE_TEST_TYPESCRIPT_LSP="$tools_dir/node_modules/.bin/typescript-language-server" \
FORGE_TEST_TYPESCRIPT_ROOT="$PWD/fixtures/typescript" \
cargo test --locked -p forge-desktop talks_to_real_typescript_server -- --ignored
```

Esta prueba usa el servidor real con `--stdio` y comprueba consulta, renombrado y completado.

### DAP con LLDB

En macOS, ejecuta la prueba con las herramientas de Xcode.

```sh
probe_dir="$(mktemp -d)"
cp fixtures/lldb/sample.c "$probe_dir/sample.c"
xcrun clang -O0 -g "$probe_dir/sample.c" -o "$probe_dir/sample"
FORGE_TEST_DAP="$(xcrun --find lldb-dap)" \
FORGE_TEST_DAP_ROOT="$probe_dir" \
cargo test --locked -p forge-desktop talks_to_real_lldb -- --ignored --nocapture
```

En macOS Apple Silicon se verificó el lanzamiento con LLDB de Xcode, un breakpoint real, la lectura de `answer = 42`, la continuación y la desconexión. Tras desbloquear la sesión se verificó también el flujo visual de editar, guardar, cerrar y reabrir un archivo. El modo Attach por PID requiere una prueba específica.

## Qué falta para 1.0

| Área | Pendiente |
|---|---|
| Editor y espacio de trabajo | Validación de IME, lector de pantalla y rendimiento en los tres sistemas. Soporte completo de reglas de exclusión, varias raíces y virtualización de texto. |
| LSP | Instalación de perfiles, varios documentos por servidor, comandos de acciones y soporte completo de snippets. |
| Debugger | Validar Attach en sistemas objetivo, depuración Node y supervisión completa del árbol de procesos. |
| Terminal y Git | Validar ConPTY y Wayland/X11. Ampliar compatibilidad de representación VT y verificar condiciones de concurrencia con procesos Git externos. |
| Extensiones y publicación | API de contribuciones ampliada y publicación de extensiones. Firma de producción, notarización, actualizaciones y pruebas nativas de instaladores. |

## Criterios para una comparación de rendimiento

Para comparar ForgeIDE con VS Code, Cursor y Zed, fija la versión de cada IDE, registra el hardware y usa la misma copia de un proyecto Rust y otro TypeScript. Mide el tiempo desde abrir hasta editar, latencia de entrada mientras se indexa, memoria estable con los mismos servidores, duración de renombrado en varios archivos y recuperación tras terminar el proceso. Ejecuta 20 aperturas en frío y documenta la configuración de cada extensión. Publica mediana y p95 junto con los casos donde el editor o el servidor no completaron la tarea.

Los resultados funcionales de esta entrega establecen qué flujos funcionan. Una comparación de velocidad, consumo o cobertura requiere ejecutar ese protocolo en condiciones equivalentes.
