# Barra de herramientas y explorador — 0.6.0

Esta entrega conserva los siete paneles y añade accesos, estados y flujos nativos. Incluye todo el trabajo de Git/GitHub de [0.5.0](GIT.md) y el gestor de [extensiones](EXTENSIONS.md).

## Accesos

En macOS, Command corresponde a Cmd; en Windows/Linux, a Ctrl en la tabla.

| Acción | Atajo de ForgeIDE |
|---|---|
| Explorador | Command+Shift+E |
| Búsqueda del espacio | Command+Shift+F |
| Git | Command+Shift+G |
| Depuración | Command+Shift+D |
| Extensiones | Command+Shift+X |
| Pruebas | Command+Alt+T |
| Lenguajes | Command+Alt+L |
| Problemas | Command+Shift+M |
| Mostrar/ocultar panel lateral | Command+B |
| Foco en el panel | Command+0 |
| Foco en el editor | Command+1 |
| Iniciar/continuar depuración | F5 |
| Detener depuración | Shift+F5 |
| Alternar breakpoint del cursor | F9 |
| Step over / in / out | F10 / F11 / Shift+F11 |

Pulsar de nuevo el icono seleccionado oculta el panel. Seleccionar otro icono lo muestra. El estado de visibilidad se guarda. Los comandos de vista, cuentas y problemas también están en la paleta.

El cambio a búsqueda dirige la escritura al buscador; se comprobó que no modifica el documento que tenía el foco. El foco de otros paneles permite entrar a sus controles con Tab. Los atajos de depuración no se consumen dentro de la terminal. La distribución completa y los mapas de teclas editables todavía requieren trabajo.

## Explorador

Los menús contextuales de archivos y carpetas y el menú superior ofrecen:

- Crear un archivo o una carpeta. La ruta es relativa a la carpeta del espacio de trabajo y su directorio padre debe existir.
- Renombrar o mover dentro de esa carpeta.
- Copiar ruta relativa o completa.
- Mover a recuperación y buscar/restaurar elementos retirados.

Se muestran las carpetas vacías y se ordenan carpetas antes que archivos. Los directorios cerrados y los filtros conservan su comportamiento anterior.

El movimiento utiliza una operación exclusiva del sistema operativo: un destino existente no se reemplaza. Las rutas no pueden salir de la raíz ni modificar .git; este flujo no sigue enlaces simbólicos. Al mover un archivo abierto se conserva el búfer sin guardar y se actualiza su ruta de recuperación.

**Move to recovery** utiliza un archivo de recuperación propio de ForgeIDE, separado de la papelera del sistema. No elimina definitivamente el elemento. Bloquea la acción si hay documentos afectados con cambios sin guardar. Si aparece una edición durante la operación, el documento se mantiene abierto con el estado de cambio externo.

Para restaurar, abre Explorer → menú → Removed items → Find removed items y selecciona el elemento. Se conserva su contenido y un destino ya existente bloquea la restauración. No hay purga automática. Los movimientos de directorios entre volúmenes pueden ser rechazados; no se recurre a borrar/copiar de forma insegura. Conserva tus copias de seguridad habituales; la recuperación de ForgeIDE no es una copia independiente del mismo disco.

## Búsqueda

Conserva regex, distinción de mayúsculas, filtros de rutas, varias raíces, exclusiones y búsqueda sobre búferes abiertos. Los reemplazos se revisan por fragmentos. Los resultados parciales permanecen marcados; no se presenta una búsqueda incompleta como exhaustiva.

## Lenguajes y problemas

**Detect installed language tools** busca herramientas instaladas para los grupos de Rust, TypeScript/JavaScript, Python, C/C++, Go y JSON. No descarga ni ejecuta servidores hasta que se solicita iniciarlos. Los perfiles personalizados siguen disponibles.

Problems reúne los diagnósticos publicados para los documentos que mantiene cada grupo de lenguaje, con navegación al archivo/línea. El icono de lenguajes y la barra de estado muestran su recuento. Un cero sin servidores iniciados no demuestra que el proyecto no tenga errores. No se implementa todavía un índice global de diagnósticos para todos los documentos.

Se corrigió la resolución de ejecutables para conservar el nombre de invocación de shims y enlaces. Los procesos reciben rutas comunes de herramientas incluso cuando Finder inicia la app con un PATH reducido.

## Depuración

Se añadieron F5/F9/F10/F11 y Shift+F5/Shift+F11, estado activo/pausado y navegación al archivo del frame detenido. F5 utiliza el perfil cargado o .forge/launch.json; alternativamente usa el adaptador/programa configurado en el panel. El motor DAP conserva variables expandibles, watches, evaluación y breakpoints del margen.

La ejecución usa archivos guardados y el build seleccionado; el panel informa si hay documentos sin guardar. Los perfiles Node/Python y Attach requieren validación específica con sus adaptadores.

## Pruebas y tareas

Sin .forge/tasks.json se sugieren tareas a partir de Cargo.toml, go.mod, pytest.ini/conftest.py y scripts test/build/dev de package.json. Una configuración explícita prevalece; si es inválida no se reemplaza silenciosamente por detección.

Las sugerencias no crean archivos ni ejecutan comandos por sí solas. El panel muestra estados y fallos, y señala cambios posteriores en los documentos. El icono de pruebas muestra fallos; el de extensiones, actualizaciones disponibles sin fijar.

Esto organiza pruebas por tarea, no descubre cada función de prueba de todos los frameworks. Los procesos y su salida tienen los límites existentes; la supervisión completa de sus descendientes sigue pendiente.

## Verificación

Las pruebas nuevas cubren movimientos sin reemplazo, recuperación y colisiones al restaurar, límites de raíces/enlaces, carpetas vacías, renombrado de un búfer sin guardar, escritura dirigida al buscador, breakpoints con Unicode, tareas detectadas y ejecutables con shims.

Consulta [VERIFICATION.md](VERIFICATION.md) para los resultados de esta compilación y [PARIDAD.md](PARIDAD.md) para las diferencias pendientes con un IDE maduro.
