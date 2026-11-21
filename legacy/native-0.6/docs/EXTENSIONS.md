# Extensiones de ForgeIDE 0.4.0

## Uso

Abre **Extensions** desde la barra lateral, incluso sin proyecto abierto.

- **Installed**: filtra por nombre o identificador y selecciona una extensión para ver sus contribuciones, README, historial de cambios e integridad.
- **Discover**: busca en Open VSX o usa **Find by ID** con publisher.name o publisher.name@1.2.3. Consulta versiones, licencia y autor. **Inspect & install** descarga y presenta las contribuciones compatibles antes de instalarlas.
- **Install file…**: acepta VSIX, paquetes portátiles .forge-extension y manifiestos forge-extension.json.
- **Updates**: comprueba versiones y actualiza una o todas las extensiones sin fijar. El menú permite comprobar actualizaciones al arrancar. Está desactivado inicialmente; no hay tráfico del catálogo hasta buscar o activar la comprobación.
- **Enabled globally** controla la extensión en toda la aplicación. **Enabled in this workspace folder** la excluye de la carpeta activa sin afectar otras carpetas. Una desactivación global prevalece.
- **Pin this version** impide las actualizaciones ofrecidas por el gestor. Puedes inspeccionar otra versión y reemplazarla expresamente.
- **Restore previous version** intercambia la versión instalada con su copia anterior. La actualización conserva activación, exclusiones por carpeta y versión fijada.
- **Uninstall** conserva una copia recuperable. Usa el menú **Restore uninstalled** para reinstalarla.
- **Export extension profile…** guarda las contribuciones y módulos, versiones y preferencias de activación en un archivo portable. No exporta rutas de proyectos. **Import extension profile…** valida todo el archivo y muestra una revisión antes de restaurar. Si falla una escritura, el informe indica qué extensiones se restauraron; la operación es atómica por extensión.
- Las extensiones nativas registran sus comandos en la paleta (Cmd/Ctrl+Shift+P) cuando hay un documento abierto y sus dependencias están disponibles.

Las búsquedas, descargas e inspecciones se ejecutan fuera del hilo de interfaz. **Cancel** descarta el resultado y detiene la cola; una petición HTTP en curso puede terminar antes de liberar el hilo. No se instala el resultado cancelado.

## Compatibilidad real con VS Code

| Aporte | Implementado |
|---|---|
| Catálogo | Open VSX: búsqueda paginada, ficha, versiones y descarga HTTPS. |
| VSIX de temas | Importación de colores de interfaz y ocho grupos de sintaxis a la paleta nativa. Includes relativos, JSON con comentarios y comas finales. |
| VSIX de snippets | Prefijos simples o múltiples, cuerpos de texto o listas, lenguajes conocidos y extensiones declaradas. Campos numéricos, valores iniciales y elección inicial del motor nativo. |
| Extension packs | Lista de miembros e inspección individual desde el gestor. No se activan silenciosamente sus miembros. |
| Paquete mixto | Solo se importan las contribuciones compatibles y se informa de las demás. |
| Extensión exclusivamente ejecutable | Se rechaza si no contiene temas, snippets ni un pack importable. |
| JavaScript/TypeScript de VS Code | No se ejecuta main/browser ni existe un host compatible con toda la API vscode. |
| Lenguajes, depuradores, vistas, webviews y comandos VS Code | No se importan como proveedores ejecutables. Los servidores de lenguaje y adaptadores de ForgeIDE siguen configurándose en sus perfiles propios. |
| Marketplace de Microsoft | No se utiliza como registro en esta entrega. |

No es compatibilidad universal con extensiones VS Code. Un archivo VSIX no garantiza que sus funciones ejecutables puedan funcionar en ForgeIDE. Las fichas enumeran contribuciones omitidas; los snippets con variables, transformaciones, campos anidados o ámbitos no admitidos se descartan con un recuento. Los colores se adaptan a los roles del renderizador nativo: no se reproduce cada propiedad de un tema de VS Code.

Open VSX verifica algunos namespaces; esa indicación procede del registro y no equivale a una auditoría de la extensión. El SHA-256 comprueba que la descarga coincide con la suma publicada por el registro cuando está disponible; no es una firma del autor.

Referencias de los formatos: [manifiesto de VS Code](https://code.visualstudio.com/api/references/extension-manifest), [contribution points](https://code.visualstudio.com/api/references/contribution-points) y [API de Open VSX](https://github.com/eclipse-openvsx/openvsx/wiki/Registry-API).

## SDK nativo, API 1.0

El ejemplo completo está en fixtures/extensions/toolbox: un comando WebAssembly, un tema y un snippet Rust. Se incluye el módulo compilado y su fuente WAT en fixtures/extensions/uppercase-first.wat.

Instala su forge-extension.json. Abre un documento corto y ejecuta **Toolbox: Uppercase first letter** desde la ficha o la paleta. Revisa la propuesta antes de aplicarla. Para el snippet escribe answer en un archivo .rs y pulsa Tab.

El manifiesto acepta schema_version, id, name, version SemVer, engine (requisito SemVer para la API, separado de la versión de la aplicación), publisher, license, description, snippets, themes, commands, dependencies, extension_pack y transform opcional para paquetes anteriores.

### Manifiesto de ejemplo

~~~json
{
  "schema_version": 1,
  "id": "forge.toolbox",
  "name": "Forge Toolbox",
  "version": "1.0.0",
  "engine": "^1.0",
  "description": "Example native extension with a palette command, a theme and a Rust snippet.",
  "publisher": "ForgeIDE",
  "license": "MIT",
  "commands": [
    {
      "id": "uppercase-first",
      "title": "Toolbox: Uppercase first letter",
      "module": "uppercase-first.wasm"
    }
  ],
  "snippets": [
    {
      "prefix": "answer",
      "extensions": [
        "rs"
      ],
      "body": "fn answer() -> i32 {\n    42\n}"
    }
  ],
  "themes": [
    {
      "name": "Toolbox Midnight",
      "dark": true,
      "colors": {
        "editor": [
          15,
          21,
          32
        ],
        "panel": [
          21,
          29,
          43
        ],
        "accent": [
          113,
          174,
          255
        ]
      },
      "syntax": {
        "keyword": [
          184,
          151,
          255
        ],
        "string": [
          151,
          206,
          157
        ]
      }
    }
  ]
}
~~~

Los módulos se leen durante la preparación, se validan y se incorporan al paquete instalado. Sus rutas deben ser relativas y permanecer dentro de la carpeta del manifiesto, incluso después de resolver enlaces simbólicos. Los comandos no se ejecutan al instalar ni al arrancar.

Las dependencias nativas usan identificadores: si alguna falta, está desactivada o forma un ciclo, no se activan las contribuciones dependientes. El gestor muestra dependencias ausentes o desactivadas. Instala sus paquetes explícitamente. No hay instalación automática de código.

### Crear un archivo portable

~~~sh
cargo build --release --locked -p forge-desktop
target/release/forgeide --extension-package fixtures/extensions/toolbox/forge-extension.json toolbox.forge-extension
target/release/forgeide --extension-inspect toolbox.forge-extension
~~~

En Windows usa target/release/forgeide.exe. En macOS instalado, el ejecutable está dentro de ForgeIDE.app/Contents/MacOS/forgeide. El empaquetador no reemplaza un archivo existente. El inspector produce un informe JSON sin abrir la ventana ni instalar nada. También inspecciona archivos VSIX.

El archivo .forge-extension incluye los módulos; no es necesario distribuir archivos Wasm por separado. Su inspección y la instalación comprueban las mismas reglas que el manifiesto local. El catálogo Open VSX se utiliza para VSIX; la publicación de paquetes nativos se realiza distribuyendo el archivo portable. No hay servicio de publicación ni sincronización de cuentas.

### Temas y snippets

Los colores RGB de interfaz admiten rail, panel, editor, elevated, border, text, muted, dim, accent, active, green y red. syntax admite comment, string, constant, keyword, function, type, property y operator. Las claves omitidas usan la paleta base clara u oscura.

El tema aplicado se copia a las preferencias y puede conservarse después de desactivar o desinstalar la extensión. Settings → Use built-in theme restaura el tema integrado.

Los snippets se seleccionan por extensión de archivo y prefijo. Los paquetes disponibles se recorren por ID; el primer prefijo coincidente prevalece sobre el integrado. El editor permite navegar por los campos. No ofrece las variables y transformaciones completas de TextMate ni espejos editables vinculados.

## Ejecución y seguridad

El comando recibe exclusivamente el texto UTF-8 del documento activo cuando lo solicita el usuario. Se ejecuta en un proceso separado con Wasmi y sin importaciones, incluida WASI: no puede solicitar archivos, red, procesos ni acceso directo a la interfaz. La salida siempre pasa por la revisión de cambios existente y sus comprobaciones de versión del documento.

Límites de la ABI actual: módulo 256 KiB, entrada/salida 8 KiB, memoria lineal 2 MiB, dos millones de unidades de combustible y terminación del proceso después de tres segundos. Esta API sirve para transformaciones pequeñas de texto; no constituye un host de plugins de propósito general.

| Exportación | Firma | Contrato |
|---|---|---|
| memory | memoria Wasm | Área para entrada y salida. |
| forge_alloc | (i32 longitud) → i32 | Devuelve el desplazamiento de escritura de la entrada. |
| forge_transform | (i32 desplazamiento, i32 longitud) → i64 | 32 bits altos: desplazamiento de salida. 32 bits bajos: longitud. |

La instancia se descarta al terminar. Se comprueban límites, UTF-8 y el estado del documento antes de aplicar el resultado.

Los VSIX nunca se extraen al sistema de archivos ni ejecutan scripts de instalación. Se rechazan rutas inseguras, enlaces simbólicos, entradas duplicadas, más de 5.000 entradas, archivos comprimidos de más de 32 MiB o expansión declarada de más de 64 MiB. Las contribuciones JSON tienen un máximo de 2 MiB, y README/changelog de 256 KiB cada uno. Se acotan la profundidad de includes, el número de temas/snippets y el tamaño del paquete persistido.

La persistencia usa un archivo temporal sincronizado y reemplazo atómico por extensión, un bloqueo entre ventanas y comparación del estado anterior para evitar sobrescribir cambios de otra ventana. El hash mostrado corresponde al archivo de origen (VSIX, manifiesto nativo o paquete portable) y se conserva como referencia en los perfiles. Los paquetes corruptos no se activan; se informa del error al cargar. Se admite un máximo de 100 extensiones instaladas. Las copias anterior y desinstalada no son un historial ilimitado.

## Comprobaciones

~~~sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --locked -p forge-desktop open_vsx_search_download_checksum_and_import -- --ignored --nocapture
~~~

La última prueba requiere internet: consulta Open VSX, descarga jdinhlife.gruvbox@1.29.2, comprueba su suma e importa sus temas. Las pruebas ordinarias usan archivos temporales; no cambian las extensiones del usuario.

## Alcance pendiente

Un host compatible con vscode, proveedores nativos de lenguaje/depuración, vistas arbitrarias, webviews, keybindings de extensiones, activación por eventos, publicación alojada, autoactualización continua y sincronización con cuentas requieren APIs adicionales. Las compilaciones y pruebas nativas en Windows y Linux, lector de pantalla, IME y uso prolongado siguen siendo necesarias para declarar una versión estable.
