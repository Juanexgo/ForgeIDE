# Ampliación vigente 0.8.1

RPC añade planDependencies(session,group,profile,base), prepareDependencies(session,group,profile,base,expectedDigest) y cancelDependencyPreparation(session). No se registran como herramientas de IA. La confirmación visual muestra registros/paquetes y liga la descarga a un digest; el backend lo recalcula antes de usar red.

El runner inicia un contenedor con un proceso fijo sleep 240 y ejecuta los tests mediante docker exec: hasta 120s de arranque y 60s de ejecución. Si el arranque falla no se informa éxito ni se ejecutan tests. Se conserva el aislamiento y se elimina solo el contenedor propio.

Los perfiles fijan la base oficial por RepoDigest y la imagen resultante por ID sha256. Solo un ID registrado localmente con el mismo perfil y digest de manifiestos puede verificar. El recibo guarda dependencyProof/workdir. No se montan dependencias del host ni claves. Ver DEPENDENCY-PROFILES.md para límites y diferencias entre la preparación con red y las pruebas aisladas.

La cancelación del cliente manda SIGTERM y fuerza SIGKILL a los dos segundos. La cancelación de BuildKit no garantiza eliminar sus capas/caché; no se hace una poda global. No hay garantía contra un administrador del host ni contra modificaciones deliberadas del estado local por el mismo usuario.

Las afirmaciones históricas de “no se hicieron llamadas reales a modelos” de versiones anteriores ya no describen 0.8.1: se hicieron ensayos locales con resultados explícitos en VERIFICATION.md. No se envió código de usuario a proveedores externos.

# Contratos vigentes 0.8

Esta sección reemplaza cualquier indicación histórica de “sidecar previsto” o “multiarchivo pendiente” más abajo.

RPC /services/forgeide: contrato fuente en packages/workbench/src/common/forge-protocol.ts. Añade groups/group/draftGroup/verifyGroup/applyGroup/revertGroup; gitState/gitAct/gitDiff/githubAccount/githubLogin/githubCancelLogin; languageTools y dependencyInventory/scanDependencies/cancelDependencyScan. La sesión pertenece a una raíz local canónica. El workspace debe ser el toplevel del repositorio para Git.

Sidecar engine/src/main.rs: un request JSON por proceso vía stdio, máximo 16MiB de entrada. {version:1,root,state,action,id?,group?}. Responde {ok:true,result} o {ok:false,error:{code,message}}. El wrapper comprueba ok, salida acotada y timeout. Acciones: draft/create/get/list/recover/apply/revert. Errores de dominio pueden tener exit 0: no interpretarlo como éxito sin leer ok.

Cambios: hasta 32 archivos, UTF-8 sin surrogates inválidos, 1MiB por contenido/8MiB por grupo. before:null crea; after:null elimina. Baseline capturado por motor. Journals privados, lock del kernel por estado, fsync y recuperación de proceso. Se rechazan rutas protegidas, escapes/symlinks, aliases case/NFC y colisiones de topología. No se garantiza transacción global contra escritores no cooperantes, pérdida de energía o ACL no probadas. Modos Unix conservados; garantías Windows necesitan pruebas.

Todas las ventanas conectadas consultan sus buffers antes de escribir. Una ventana sucia o que no responda bloquea la operación. La consulta no congela el editor ni otros programas. Conflictos se informan y se preserva trabajo externo. applyGroup(verified=true) exige recibo aprobado exacto y snapshot vigente; verified=false es una operación manual explícita, nunca una aprobación automática.

Las herramientas de agente no permiten aplicar, revertir, Git, shell del host ni escaneo de red. La nueva herramienta de verificación usa solo los dos perfiles de contenedor y exige permiso de la política Theia. Los agentes genéricos de Theia tienen políticas distintas: no heredan automáticamente estas restricciones.

Git usa argumentos sin shell, limita salida/tiempo, deshabilita fsmonitor/protocolo ext/file y hooks salvo opt-in explícito para commit/merge. Nunca usa push --force. gh administra las credenciales; Forge no copia tokens. La GUI nativa debe aún validar cada paso del login real. Trust del workspace es por sesión.

Quality requiere consentimiento para enviar únicamente inventario de nombres/versiones a npm/OSV. No envía código. Detecta cambios del lockfile antes de la llamada, y resultados obsoletos después. Los fallos de red/parseo no producen un veredicto limpio. Registro soportado limitado: package-lock, Yarn v1, Cargo crates.io y requirements exactos. Los mirrors/paquetes privados se omiten con aviso.

Graph corre en worker thread, cola máxima4, timeout30s y heap192MiB. No es sandbox contra código hostil: solo se analizan fuentes como datos. Snapshot para tests excluye dotfiles; no equivale al proyecto entero ni instala dependencias.

## Contratos históricos y controles conservados

# Contratos y seguridad

## RPC activo

Endpoint `/services/forgeide`, contrato TypeScript en `packages/workbench/src/common/forge-protocol.ts`. Reutiliza transporte y protección de conexión de Theia/Electron. El lanzador fija loopback si el usuario no proporciona otro hostname; el producto desktop no debe publicarse como servidor multiusuario.

| Método | Entrada | Salida / garantía |
|---|---|---|
| open | URI `file:` local | session opaca por conexión, primera raíz seleccionada por UI |
| proposals | session | historial persistente de esa raíz |
| propose | session, ruta relativa, contenido completo | propuesta review, verification=not_run; no toca archivo |
| apply | session, ID | aplica solo si disk coincide exactamente con before |
| revert | session, ID | restaura solo si disk coincide exactamente con after |
| graph | session | nodos y aristas, no ejecuta archivos, límites explícitos |
| health | session | datos reales del backend, no resultados de auditoría |

Una desconexión invalida las sesiones. Errores RPC se muestran en el panel. El almacenamiento del historial se ubica en `~/.forgeide/safeapply/<sha256 de ruta canónica>`; `FORGE_STATE_DIR` permite aislarlo en tests. Propuestas incluyen contenido anterior y nuevo: pueden contener datos privados. Directorios 0700 y archivos 0600 donde POSIX aplica. Los permisos equivalentes de Windows deben verificarse antes de beta.

## Modelo de amenaza alpha

Protege frente a errores de rutas, propuestas obsoletas, doble aplicación y reversión que borraría cambios posteriores. Rechaza rutas absolutas, traversal, symlinks, `.git`, `.forgeide`, `.env*`, llaves comunes, binarios e input superior a 1 MiB. Estos filtros no identifican todos los secretos posibles: el contexto enviado a IA debe revisarse aparte.

No protege contra otro proceso malicioso ejecutándose como el mismo usuario, sustituciones concurrentes de directorios ni plugins Node con acceso al sistema. No es un sandbox. El grafo no ejecuta source, pero los parsers y plugins son dependencias que requieren actualizaciones y auditoría.

SafeApply usa un lock exclusivo por workspace, snapshots en journal y reemplazo mediante rename. Registra intención antes de modificar disk y reconcilia al reabrir. Un lock abandonado se conserva deliberadamente: cerrar todas las instancias, inspeccionar el journal y resolverlo antes de quitar `operation.lock`. No hay transacciones multiarchivo. Se preservan bits de modo; ACL, atributos extendidos y garantías ante corte eléctrico requieren implementación/validación adicionales. El journal no promete fsync completo de directorios en todos los SO.

El panel bloquea un editor dirty en la ventana actual. Todavía debe coordinar buffers de múltiples ventanas y autosave con una transacción del editor. No usar esta alpha para aplicar cambios concurrentes a archivos con trabajo sin guardar en otra ventana. La precondición en disk cubre cambios guardados, no buffers invisibles.

## IA

`forge_safeapply_propose` está registrado como ToolProvider Theia AI. Solo crea propuestas y devuelve ID/estado; no expone apply/revert al modelo. La política de confirmación es la de Theia AI, sin autoaprobación añadida. Los agentes genéricos de Theia pueden ofrecer otras herramientas con sus propios permisos: el aislamiento de SafeApply no restringe mágicamente esas herramientas.

No se configuraron proveedores ni se transmitió source a modelos durante esta entrega. Un modelo local puede elegirse mediante Ollama. Configurar un proveedor remoto implica revisar sus condiciones, contexto, retención y costos. No incluir claves en repositorio, logs, métricas o fixtures. Los tokens GitHub deben gestionarse mediante el proveedor/almacén del SO; jamás copiar los de legacy.

## Contrato sidecar previsto (aún no implementado)

JSONL sobre stdio o canal local autenticado, versión `forge/1`, mensajes `{id,method,params}` y respuestas `{id,result}` o `{id,error:{code,message,retryable}}`. Máximo por frame, timeout, cancelación y backpressure obligatorios. Sesión ligada al workspace canónico; ningún shell arbitrario o ruta absoluta aceptados por defecto. Cambios incluyen expected buffer version, sha256 y permisos. La caída del proceso invalida operaciones activas, jamás informa éxito.

## Extensiones y distribución

Host de extensiones separado por estabilidad, no aislamiento de privilegios. Para extensiones de terceros: versión fijada, licencia revisada, checksum y prueba de contribuciones editor/LSP/DAP/webview. Tener presente compatibilidad de API, arquitectura y sistema.

Firma/notarización y actualización automática no están activas en la alpha. Antes de publicación: auditoría de dependencias, triage de findings, SBOM, avisos legales, firma y testeos de actualización. No usar el archivo MIT del código propio para describir todas las dependencias o los builtins.

## Ampliación 0.7.1: verificación real y agentes

El RPC añade `prepareRuntime`, `read`, `runs`, `verify`, `cancelVerification` y `applyVerified`. Los métodos de ejecución y aplicación no se registran como herramientas de los agentes Forge. `prepareRuntime` es una acción manual de UI que descarga la imagen oficial Node y resuelve su digest. `verify` solo acepta imágenes `node@sha256:<64 hex>` ya instaladas y usa `--pull=never`.

El runner monta una copia filtrada del workspace como readonly, sin `.env`, dotfiles, claves conocidas, symlinks, dependencias y outputs. Límites de snapshot: 2000 archivos, 2 MiB por archivo y 32 MiB total. Un archivo demasiado grande bloquea la ejecución; no se simula una prueba sobre un snapshot incompleto. Los filtros no detectan todos los secretos embebidos en código. La selección del contexto de IA sigue siendo una decisión separada.

Contenedor: `--network=none`, `--read-only`, `--cap-drop=ALL`, `--security-opt=no-new-privileges`, `--user=1000:1000`, 512 MiB, 1 CPU, 128 pids, timeout 60 segundos y /tmp tmpfs de 64 MiB sin ejecución. Docker proporciona aislamiento de contenedor; no se promete protección absoluta frente a fallos del kernel/runtime. No se monta el socket Docker ni credenciales del host. Solo la descarga explícita del runtime usa red.

El éxito exige exit code cero y al menos una prueba aprobada según el reporte TAP de Node. Un proyecto controla sus pruebas y puede imprimir datos engañosos: el recibo demuestra el comando y su resultado, no una prueba formal de corrección. Las salidas se limitan a unos 64 KiB. Se guarda identidad de imagen, comando, hashes de propuesta/snapshot y resultado en `verification.sqlite` (WAL, schema 1). Los snapshots temporales se eliminan tras la ejecución. La cancelación intenta detener y eliminar únicamente el contenedor con nombre UUID creado por esta ejecución.

`applyVerified` compara propuesta y snapshot actual con el último recibo. Si cambió cualquier input incluido, exige nuevas pruebas. No es una transacción global del filesystem: otros procesos podrían modificar archivos después de la comprobación. Continúan pendientes la coordinación de todas las ventanas, ACL, multiarchivo y pruebas exhaustivas de caídas/disco lleno. La aplicación manual “Aplicar sin pruebas” sigue disponible y no crea un recibo aprobado.

Los agentes Forge Implement/Tests/Security/Fix filtran sus herramientas en código en cada llamada al modelo. Sus capacidades son leer archivo, grafo, recibos y crear propuestas. No pasan herramientas del proveedor ni settings arbitrarios del chat. Los agentes genéricos de Theia siguen siendo capacidades distintas y no quedan sujetos automáticamente a esta lista.

La verificación y la descarga del runtime rechazan contextos Docker remotos (`tcp`, `ssh` y named pipes remotos). Se exige un daemon local: un bind mount en otro host no representaría el snapshot local verificado. Se añadieron tests de este límite, de preservación de BOM/CRLF al revertir y de rechazo de reemplazos con surrogates Unicode inválidos.
