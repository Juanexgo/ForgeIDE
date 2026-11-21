# Perfiles de proyecto en ForgeIDE 0.3.0

## Tareas

En Terminal abre **Project tasks**, pulsa **Load project tasks** y selecciona una tarea. Cada ejecución abre otra terminal y muestra el código de salida al terminar. Los programas reciben los argumentos como elementos separados.

Crea .forge/tasks.json con este formato.

~~~json
{
  "version": 1,
  "tasks": [
    { "label": "Probar Python", "program": "python3", "args": ["-m", "unittest", "discover"] },
    { "label": "Comprobar Rust", "program": "cargo", "args": ["check", "--locked"], "cwd": "." }
  ]
}
~~~

El programa acepta un ejecutable instalado en PATH o una ruta. El directorio cwd es opcional y se resuelve desde la carpeta abierta. La interfaz identifica los ejecutables ausentes. En aplicaciones abiertas desde Finder, configura rutas absolutas si el PATH del sistema omite tus herramientas.

## Exclusiones

Crea .forge/excludes y actualiza Explorer. Las reglas se aplican al listado y a las búsquedas que usan ese listado.

~~~text
# Archivos generados
build/
**/*.generated.ts
src/**/cache/
~~~

Se admiten *, ** y ?. Las carpetas .git, node_modules y target se excluyen siempre. Las reglas con ! necesitan implementación adicional. Este archivo tiene semántica propia y convive con .gitignore.

## Lenguajes

Crea .forge/languages.json.

~~~json
{
  "version": 1,
  "servers": [
    { "extension": "rs", "program": "rust-analyzer", "args": [] },
    { "extension": "ts", "program": "/ruta/typescript-language-server", "args": ["--stdio"] },
    { "extension": "py", "program": "/ruta/pyright-langserver", "args": ["--stdio"] }
  ]
}
~~~

En Language Server carga el archivo, selecciona cada grupo e inicia su servidor. Los grupos tienen procesos y solicitudes independientes. Cada proceso conserva el último documento de su grupo y recibe los cambios de ese búfer. La edición simultánea de varios documentos dentro de un mismo grupo requiere ampliar la sincronización.

Los marcadores de diagnóstico muestran el mensaje al situar el puntero sobre ellos. El menú contextual del editor ofrece referencias, definición, acciones de código y formato. Las operaciones disponibles dependen del servidor. Las acciones que incluyen comandos adicionales requieren soporte de extensiones.

## Adaptadores de depuración

Crea .forge/launch.json. Los argumentos se envían al adaptador. Las cadenas admiten los marcadores de carpeta y archivo activo del ejemplo.

Este perfil requiere Python con debugpy instalado.

~~~json
{
  "version": 1,
  "configurations": [
    {
      "name": "Python — archivo activo",
      "adapter": "/ruta/python3",
      "adapter_args": ["-m", "debugpy.adapter"],
      "request": "launch",
      "arguments": {
        "type": "python",
        "program": "${file}",
        "cwd": "${workspaceFolder}",
        "console": "internalConsole",
        "justMyCode": true
      }
    }
  ]
}
~~~

Para Node configura el ejecutable Node, la ruta del adaptador DAP de JavaScript y los argumentos de lanzamiento que admite ese adaptador. ForgeIDE permite solicitudes launch y attach. Los perfiles necesitan validación con las versiones concretas de los adaptadores.

En Debugger abre **Project debug profiles**, carga el archivo y pulsa **Start profile**. El adaptador recibe los breakpoints del archivo activo. Los cambios de breakpoints de otros archivos se envían al accionarlos desde su margen. El cliente admite hasta 20 watches y rechaza solicitudes inversas de ejecución de terminal.

## Historial y revisión

El menú contextual abre **Local file history**. Selecciona una versión para revisar su restauración. Aplicarla modifica el búfer y guardar confirma el cambio en disco. Se conservan hasta 20 versiones previas por archivo.

Las propuestas de reemplazo, renombrado, formato, acciones de código y resolución de conflictos presentan casillas para seleccionar fragmentos. El historial permite deshacer las operaciones aplicadas. La revisión vuelve a comprobar el contenido antes de cambiar los búferes.

La preparación de fragmentos Git se acciona desde **Working diff**. ForgeIDE compara el diff antes de preparar y pide abrirlo de nuevo cuando cambió. Los cambios binarios y de modo requieren la operación de archivo completo.

## Espacios con varias carpetas

Explorer permite añadir y quitar carpetas desde su menú. Quitar una carpeta conserva los archivos abiertos y las ediciones. Git, tareas, pruebas, lenguaje y depuración usan la carpeta propietaria del archivo activo. Al alternar entre carpetas se conservan los paneles de herramientas y continúan las sesiones ya iniciadas.

Los archivos de configuración y las exclusiones siguen siendo propios de cada carpeta. Los reemplazos abarcan las carpetas incluidas y vuelven a validar cada archivo. La indexación publica resultados progresivamente y mantiene el límite total de 10.000 rutas. Los reemplazos requieren que el listado esté completo.

## Panel de pruebas

Marca las tareas de prueba con el campo kind.

~~~json
{
  "version": 1,
  "tasks": [
    {
      "label": "Pruebas de Python",
      "kind": "test",
      "program": "python3",
      "args": ["-m", "unittest", "discover", "-v"]
    },
    {
      "label": "Pruebas de Rust",
      "kind": "test",
      "program": "cargo",
      "args": ["test", "--locked"]
    }
  ]
}
~~~

En Testing pulsa Load test tasks y ejecuta una tarea o Run all. Las tareas se ejecutan secuencialmente. Cada resultado muestra su salida, tiempo y estado. Stop detiene el proceso directo y cancela la cola pendiente. El control completo de procesos descendientes requiere ampliar el ejecutor.

La salida se drena para evitar bloqueos y su representación se limita a 256 KiB por tarea. Los resultados se mantienen durante la sesión. Una firma de los documentos abiertos permite avisar cuando cambian después de una ejecución. Esa firma tiene alcance sobre los documentos abiertos y su estado conocido en disco.

Las pruebas usan archivos guardados. El panel avisa de los búferes sin guardar. Cuando hay pruebas cargadas, la revisión ofrece Apply, save & test active folder. Esa acción guarda los archivos seleccionados y arranca las tareas de la carpeta activa cuando los guardados terminan correctamente.

Las tareas de prueba también funcionan en Terminal. El panel todavía presenta el resultado de cada tarea o suite; el descubrimiento de casos individuales y su depuración requieren adaptadores adicionales.
