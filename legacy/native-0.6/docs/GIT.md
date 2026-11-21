# Git y GitHub en ForgeIDE 0.5.0

## Conectar la cuenta

Pulsa el icono **GitHub accounts** al pie de la barra lateral, o abre **Source Control → Accounts**. **Sign in with GitHub…** inicia la autorización oficial de GitHub CLI en el navegador. El panel muestra la salida y el código de dispositivo si hace falta; puedes abrir manualmente la página de autorización. Completa el permiso en GitHub. ForgeIDE comprueba después la cuenta activa.

Requiere Git instalado. Para conectar GitHub también requiere [GitHub CLI](https://cli.github.com/); el gestor detecta instalaciones comunes de macOS y Windows además del PATH. Git local no necesita iniciar sesión.

**Check account** consulta las cuentas; **Use this account** cambia la activa y **Sign out…** pide confirmar la salida. Salir afecta también a otras herramientas que usen esa cuenta de GitHub CLI. Cancelar un inicio de sesión detiene el proceso local; una autorización completada en el navegador se administra desde GitHub.

Los tokens los gestiona GitHub CLI, que intenta usar el almacén de credenciales del sistema y puede usar su archivo de configuración si no está disponible. ForgeIDE no solicita contraseñas, no imprime tokens y no los copia al proyecto. Guarda únicamente la preferencia de usar la cuenta conectada. Las operaciones HTTPS para github.com usan un helper acotado al comando cuando la conexión está activada; los helpers Git existentes siguen funcionando si no se ha conectado una cuenta. SSH utiliza tu configuración y agente SSH existentes.

La identidad del autor de commits es independiente del acceso a GitHub. **Commit author → Save for this repository** configura nombre/correo únicamente en el repositorio actual. Puedes usar el correo no-reply que GitHub muestre en los ajustes de tu cuenta; ForgeIDE no inventa un correo.

## Abrir, crear o clonar

Source Control detecta el repositorio que contiene la carpeta abierta, aunque abras un subdirectorio. Muestra la raíz real para que puedas revisar todo el alcance.

Si la carpeta no pertenece a un repositorio, ofrece **Initialize repository**. **Clone repository** acepta una URL HTTPS/SSH o una ruta local absoluta: indica una carpeta nueva y elige su directorio padre. El resultado se añade al espacio de trabajo, conservando los documentos abiertos.

Una descarga fallida o cancelada puede dejar una carpeta parcial. No se elimina automáticamente; elige otra carpeta o inspecciona la anterior. No se reemplaza un destino existente.

## Revisar y confirmar cambios

El panel separa **Merge changes**, **Staged changes** y **Changes**, con estado, ruta, apertura en el editor y preparación/retirada por archivo. **Stage all** prepara los cambios guardados visibles; **Unstage all** conserva el contenido de trabajo. Los commits requieren mensaje y cambios preparados, y se bloquean si hay conflictos o rutas que no se pudieron mostrar.

Pulsa un archivo para abrir la ventana **Git changes**. Tiene comparación en dos columnas y vista unificada. En cambios de trabajo, expande **Stage individual hunks or lines**. La preparación parcial verifica que el diff siga siendo el revisado. Archivos binarios, cambios de modo y archivos nuevos usan preparación completa cuando el formato no permite un fragmento normal.

**Commit staged** confirma exclusivamente el índice de Git. La aplicación comprueba su firma y la rama revisada antes de iniciar el commit; un cambio concurrente requiere refrescar. Escribir en el editor no guarda ni prepara automáticamente. La barra de estado permite volver a Source Control desde la rama, y el icono Git muestra el número de archivos modificados.

Los conflictos se abren en el editor para usar la revisión de resolución existente. Guarda la resolución, prepárala y confirma. No se resuelven conflictos silenciosamente.

## Descartar con copia previa

**Discard saved…** se ofrece para archivos de texto ya controlados por Git. Revisa el archivo y pulsa **Archive and discard**. Primero archiva el texto guardado en Local history; después restaura el archivo desde el índice. El índice y los búferes abiertos sin guardar se conservan.

Si cambian el archivo, la rama base o el índice antes de aplicar, debes revisar de nuevo. Si falla la copia previa no se descarta el contenido. Para recuperar: abre el archivo, usa el menú contextual **Local file history…**, elige la copia, revisa su restauración y guarda.

Este flujo admite texto UTF-8 de hasta 512 KiB y excluye enlaces simbólicos y rutas que salen del repositorio. No elimina archivos nuevos sin seguimiento ni descarta binarios.

## Ramas, historial y stashes

**Branches & worktrees** permite listar, crear, cambiar y renombrar ramas locales. **Delete merged** requiere revisión y usa la eliminación normal de Git: no fuerza una rama sin fusionar ni borra la rama activa. También lista y crea worktrees con una rama nueva.

**History & compare** muestra los últimos 50 commits con autor y hash. Selecciona uno para revisar su cambio o indica dos hashes completos para compararlos.

**Stashes** guarda cambios de archivos controlados y no controlados. **Apply, keep stash** restaura el contenido y mantiene la copia para recuperación. Una aplicación con conflictos deja los archivos y el stash disponibles para resolver; no se hace pop ni drop automático. Los búferes sin guardar deben guardarse primero.

## Remotos y publicación

**Remotes & synchronization** permite añadir remotos, seleccionar destino y rama, y ejecutar Fetch, Pull, Push o Sync. Muestra tanto el destino de lectura como todos los destinos de publicación configurados. Pull, Push, publicación y Sync requieren revisión en el panel.

- Fetch actualiza referencias y limpia referencias remotas obsoletas.
- Pull usa fast-forward y rechaza historias divergentes.
- Push transmite la rama indicada, sin forzar ni incluir automáticamente etiquetas o submódulos.
- Publish branch establece su seguimiento remoto.
- Sync hace pull fast-forward y luego push; si una fase falla pueden permanecer las fases completadas.
- Sin remotos puedes crear un repositorio nuevo en GitHub, privado inicialmente, y publicar la rama. Introduce owner/repository. Hacerlo público exige marcarlo expresamente y revisar la publicación. Si la creación termina y la carga falla, se conserva el repositorio y origin para reintentar.

La rama activa y HEAD se comprueban antes de las operaciones revisadas. No hay force-push. Se rechazan URLs con credenciales incluidas, protocolos de helpers arbitrarios y nombres que puedan interpretarse como opciones.

Las operaciones son asíncronas. **Cancel Git operation** solicita terminar el comando local; las fases ya completadas no se revierten. Los comandos locales tienen un límite de 15 segundos y los de red de 120 segundos; los de cuentas, 25 segundos salvo la autorización del navegador, que puede esperar 15 minutos. La supervisión completa del árbol de procesos en todos los sistemas requiere validación adicional.

Git puede ejecutar los filtros, hooks y helpers configurados por el usuario o el proyecto durante una operación solicitada. ForgeIDE usa el ejecutable instalado, desactiva fsmonitor en sus llamadas, evita diffs externos/textconv y preserva sus comprobaciones de Git.

## Validación y alcance

La suite de 0.5.0 comprueba repositorios reales temporales: inicialización, clonado, autor local, publicación a un remoto bare local, fetch/pull/sync, divergencia, HEAD y rama desactualizados, índice sin primer commit, ramas, stashes con archivos nuevos, descarte recuperable, URLs de publicación distintas y cancelación del proceso.

En la revisión de la ventana se detectó una cuenta existente y activa almacenada en el llavero. No se inició una autorización nueva ni se creó un repositorio en GitHub durante estas pruebas. El consentimiento OAuth requiere al usuario en el navegador. La creación remota está implementada y usa GitHub CLI; requiere validación con una cuenta de prueba antes de declarar ese flujo estable.

Esta entrega no es paridad completa con VS Code: no incluye un grafo completo de commits, revisiones de pull requests, gestión de submódulos, rebase/cherry-pick interactivo ni un proveedor de GitHub Enterprise. Las pruebas nativas de Windows/Linux y jornadas largas siguen pendientes.

Referencias: [acceso con GitHub CLI](https://cli.github.com/manual/gh_auth_login), [credenciales de Git](https://cli.github.com/manual/gh_auth_setup-git), [Git push](https://git-scm.com/docs/git-push).
