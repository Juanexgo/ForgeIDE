# Evidencia 0.8.1-alpha.1

20 de noviembre de 2026, America/Merida; algunos logs corresponden al 21 de noviembre UTC. Mac Apple M2/8 GB, Node 24, Electron 44.9.1, Theia 1.77.0. Ninguna ejecución pública de GitHub Actions.

## Comprobaciones aprobadas

- 54/54 pruebas locales, cero skipped. Incluyen planes de dependencias, integridades, registros obsoletos, cancelación y rechazo de un arranque fallido antes de ejecutar tests.
- Tres integraciones Docker reales base aprobadas con el runner final: single-file Node y grupos create/edit/delete Node/Python; recepción, aplicación, reversión y rechazo de cambios posteriores.
- Dos integraciones Docker reales con bibliotecas públicas: is-number 7.0.0 y six 1.17.0. Se comprobó instalación fijada, scripts npm deshabilitados, wheels/hashes Python, ejecución offline como usuario 1000, dependencias readonly, proof persistente, apply/revert y rechazo tras modificar el lockfile.
- Las pruebas propias de contenedor conservan 60s. El arranque ahora se mide aparte (hasta 120s), con startupMs y executionMs en recibo. Un arranque bloqueado nunca equivale a tests aprobados.
- Compilación TypeScript y bundles browser/node/electron sin errores. El motor Rust no cambió: versión 0.8, protocolo1; los tres crash tests de 0.8 son evidencia histórica, no nuevos casos ejecutados en esta revisión.
- UI de la versión de prueba: reconoció cuenta gh existente; se inicializó Git desde el panel; escritura/guardado en el editor se comprobó también en disco. No se completaron stage/commit/DAP/extensiones desde la UI. La herramienta visual falló después y el Mac volvió a aparecer bloqueado.
- Auditoría Yarn repetida: 0 críticas, 0 altas, 37 moderadas y 4 bajas, cuatro advisories únicos. Informe actual en dependency-audit.json. Sigue pendiente resolver/triagear las cuatro alertas; no es certificación de seguridad.
- Inventario NPM CycloneDX actualizado (1199 componentes del lockfile), con alcance y exclusiones explícitos. No incluye un inventario completo de binarios/plugins/Cargo o relaciones/licencias verificadas.
- Benchmark de grafo actualizado. Fixture grande truncada a 1000 módulos; no demuestra indexación completa ni superioridad frente a otros IDE.

## Ensayos que no se cuentan como aprobados

Los primeros intentos de Docker fallaron por disco lleno/daemon sin responder. Se eliminaron únicamente cachés y builds temporales propios, sin restablecer Docker ni borrar imágenes/volúmenes del usuario. La integración encontró y corrigió un índice pip incompleto. Un test intentaba re-verificar una propuesta ya revertida: se corrigió el fixture creando otra propuesta.

La repetición de una prueba base mostró timeout al incluir el arranque de Docker en el tiempo de tests. Se separaron preparación y ejecución; los tres casos posteriores con el runner final pasaron. No se amplió el límite de ejecución de tests ni se permitió convertir timeouts en éxitos.

IA real local: comprobación básica Ollama aprobada. Un ensayo Llama3.2:3b leyó el archivo pero no creó propuesta. Los ensayos posteriores Llama/Qwen, incluido CPU, terminaron por timeout. Por tanto no está aprobado el recorrido autónomo read→propose→verify→apply/revert, ni se acredita calidad de los cuatro agentes. El harness usa una carpeta sintética y tiene límites explícitos de transporte para futuros ensayos. No se hicieron llamadas a proveedores externos con código del usuario. El flujo UI completo de chat/configuración/permisos no está certificado.

Se autorizó cambiar Docker de 5120 a 2048 MiB para las pruebas; la configuración se restauró a 5120 MiB al terminar. El arranque final y la integridad del paquete se registran en el manifiesto de entrega.

## Paquete final 0.8.1

- 54 pruebas locales repetidas usando el Electron distribuido: 54/54 aprobadas, sin skipped. Son los mismos casos, no 54 casos nuevos.
- PTY real, motor Rust draft/apply/revert y worker empaquetado: PASS.
- App final arrancó en un perfil aislado, inició 90 plugins y frontend llegó a ready sin errores en ese arranque. No prueba todas las funciones de cada plugin.
- DMG HFS+/UDZO: hdiutil verify VALID. ZIP de fuente y checksums se verifican al guardar la entrega.
- Paquete macOS arm64 alpha sin firma/notarización ni actualizaciones activas. Cerrar la versión anterior antes de iniciar la nueva evita que el bloqueo de instancia redirija a un proceso viejo.


## Pendientes externos y de producto

Windows/Linux, firma/notarización, actualización firmada, repositorio remoto y CI pública, onboarding OAuth/expiración/logout/push/PR reales, pruebas de UI completas, servidores de cada lenguaje, grafo de otros lenguajes, remoto, accesibilidad, cobertura/evals/mediciones de jornadas largas. No hay porcentaje fiable de “todo terminado”.

## Evidencia histórica (versiones previas)

---

# Evidencia 0.8.0-alpha.1

- Build completo: sidecar Rust release + TypeScript + bundles browser/node/electron, sin errores.
- 47/47 tests locales aprobados, cero skipped.
- 3/3 integraciones de crash reales: muerte tras primera escritura, conservación de cambios externos y muerte antes de rename con limpieza de staging durable.
- 3/3 integraciones Docker reales: single-file Node y grupos create/edit/delete Node y Python. Verificación, inputs originales intactos antes de aplicar, aplicación y reversión comprobadas.
- Auditoría Yarn archivada: 0 críticas/altas, 37 moderadas, 4 bajas; cuatro advisories únicos. No es certificación de seguridad del paquete ni incluye automáticamente todos los VSIX/nativos.
- Benchmark JS/TS actualizado en benchmarks/graph-local.json. Solo grafo, tres muestras; no demuestra superioridad ni indexación completa del caso truncado.
- UI: CUA informó que el Mac estaba bloqueado. No se simularon clics ni se contaron flujos interactivos como aprobados.
- Ningún modelo real, autorización OAuth nueva, push/PR, runner GitHub ni Windows/Linux ejecutado en esta máquina.

## Comprobaciones del paquete 0.8

- Empaquetado macOS arm64 completado sin firma.
- Binario distribuido: PTY real PASS, sidecar Rust draft/apply/revert PASS, worker de grafo extraído de asar PASS.
- Tres integraciones Docker repetidas bajo Electron distribuido: PASS. Repeticiones del mismo caso no aumentan el conteo de 53 casos distintos.
- App empaquetada arrancó en perfil separado, backend loopback y frontend llegó a ready; 90 plugins iniciados. Corregido el perfil antes del single-instance lock para no interferir con la versión anterior.
- Advertencia de módulo ESM sin type en worker (carga correcta con autodetección), avisos Theia de activación builtin no soportada; compatibilidad universal no certificada.
- UI de Git/login, depurador y flujos de clics no validados: Mac bloqueado.

Las evidencias históricas de abajo pertenecen a versiones anteriores.

---

# Verificación actual — 0.7.1-alpha.1

Resultado actual: **24 pruebas locales y 1 integración real Docker aprobadas**, build y typecheck correctos. El panel 0.7.1 se inspeccionó visualmente; la interacción completa quedó interrumpida por el bloqueo del Mac. El estado detallado de esta revisión está en la sección final y en ESTADO-COMPLETO.md. Los resultados de 0.7.0 de abajo son históricos.

# Evidencia de la migración — 0.7.0-alpha.1

Fecha de trabajo: 19 de noviembre de 2026, zona America/Merida. Algunas marcas de logs corresponden al 20 de noviembre en UTC. Entorno local macOS arm64. No se han ejecutado runners externos de GitHub Actions.

## Resultados locales

| Comprobación | Resultado |
|---|---|
| Instalación Yarn y lockfile | Correcta, Node 24.13.1 / Yarn 1.22.22 |
| `yarn build` | TypeScript y bundles browser/node/electron sin errores |
| `yarn run typecheck` | Correcto |
| `node --test tests/*.test.cjs` | 14/14 pasan, cero skipped en macOS |
| `yarn rebuild` | PTY, keytar, native-keymap y drivelist preparados para Electron |
| `yarn download:plugins` | Builtins fijados a release 1.108.2 descargados; runtime informa 90 plugins sincronizados |
| Arranque desde desarrollo | Backend en 127.0.0.1, shell adjunto, Forge Control visible en accesibilidad macOS |
| Uso del panel | Títulos, tabs y formularios observados; apertura de carpeta interrumpida al bloquearse el Mac |
| `yarn package:mac` | `.app` arm64 generada; usa Electron preparado con ffmpeg libre de codecs propietarios comprobados por Theia |
| Smoke del paquete | Backend loopback, 90 plugins sincronizados, shell en estado ready; prueba real PTY imprime marcador y pasa |
| Recursos empaquetados | Código y CSS propios incluidos; auxiliares PTY/ripgrep extraídos; ffmpeg coincide con runtime preparado |
| Windows/Linux | Workflow preparado, no ejecutado |
| OAuth GitHub / keychain / push y pull | Pendiente; ningún token copiado de legacy ni nuevos permisos concedidos |
| Pruebas de un proyecto desde UI / agentes con modelo real | Pendiente; no se ejecutaron llamadas a modelos |
| Accesibilidad integral / IME / rendimiento comparativo | Pendiente |

No se suman las pruebas de la aplicación Rust anterior a estas 14 pruebas. Los smoke tests del servicio llaman al backend real, pero no sustituyen una prueba integral de RPC y UI.

## Qué prueban los tests

- Crear propuesta sin escribir el archivo; aplicar y revertir después de reabrir historial.
- Conflictos por edición externa y después de aplicar; propuestas rivales y doble aplicación.
- Traversal, rutas absolutas, metadata, secretos conocidos, symlinks, binarios, UTF-8 inválido y tamaño excesivo.
- Lock exclusivo, validación de IDs, modo ejecutable y reemplazo vacío.
- Reconciliación de intención de escritura y divergencia tras interrupción simulada. No es aún una campaña de terminaciones de proceso en cada instrucción.
- Imports/exports/dynamic imports/require estáticos, ciclos, deduplicación, exclusiones y symlinks sin ejecutar el source.
- Sesiones del servicio, invalidación al desconectar, rechazo de workspace remoto y circuito proponer/aplicar/revertir.
- Recursos de comparación inmutables, sin método de guardado ni contenido en URIs, y apertura repetida.

En Windows dos casos de symlinks se omiten si no se usa una ejecución con privilegios adecuados. Antes de beta deben ejecutarse en un entorno Windows con soporte de symlinks; el skip no es evidencia de seguridad.

## Auditoría de dependencias

Se ejecutó `yarn audit --json`. Resultado informado por Yarn: **0 críticas, 3 altas, 173 moderadas y 65 bajas**. Los conteos incluyen rutas/dependencias repetidas y no equivalen a ese número de fallos únicos. Informe deduplicado: `dependency-audit.json`.

Las altas observadas están en cadenas de herramientas de desarrollo: `@theia/cli → mocha → serialize-javascript 6.0.2` y `@theia/cli → puppeteer → chromium-bidi → zod 3.25.76` (dos rutas). Actualizarlas forzando saltos de versión mayor requiere comprobar compatibilidad; no se han ocultado ni marcado como corregidas. Debe hacerse triage de las moderadas/bajas y separar el árbol efectivamente distribuido antes de cualquier publicación estable. Este resultado bloquea presentar la alpha como auditada y segura para producción.

## Advertencias de compatibilidad observadas

Theia informó eventos de activación no soportados por algunas extensiones builtin, entre ellos `onProfile:github`, `onOpenExternalUri:*` y eventos de shell integration. Que GitHub Authentication se descargue y el host arranque no comprueba el login. La prueba con navegador, retorno al protocolo de ForgeIDE, credenciales, cancelación y logout es obligatoria antes de beta.

La inicialización registró también advertencias de gramáticas y preferencias de extensiones. Se conservan como trabajo de compatibilidad; no se ha certificado paridad total con VS Code 1.108.2.

## Puertas para beta

1. Instalar el paquete fuera del árbol de desarrollo y comprobar edición, plugins, terminal, Git, LSP, DAP y recuperación con fixtures.
2. Validar login GitHub sin registrar tokens, identidades de commit y gestor de credenciales del SO.
3. Coordinar SafeApply con buffers y autosave de múltiples ventanas; tests de caída real, disco lleno y ACL.
4. Runner de pruebas aislado y recibos ligados al snapshot; no llamar SafeApply “verificado” antes de eso.
5. Resolver alertas aplicables, generar SBOM/avisos completos, firmar/notarizar y medir el producto.
6. Matriz real macOS/Windows/Linux. CI configurada no significa CI ejecutada ni aprobada.

## Regresión de empaquetado corregida

El primer smoke encontró `posix_spawn failed`: el helper de PTY estaba dentro de asar y no podía ejecutarse. Se añadieron `lib/prebuilds/**` y auxiliares nativos a `asarUnpack`, se reempaquetó y se repitió el arranque. `scripts/check-packaged-pty.cjs` ejecutó una shell real desde el Electron distribuido y obtuvo `Packaged PTY: PASS`. El segundo arranque llegó a ready sin ese error. La interacción visual con terminal, selección, colores y teclado sigue pendiente.

# Revisión 0.7.1-alpha.1

Esta sección reemplaza el estado anterior en lo relativo a runner, agentes y SQLite. La revisión histórica de 0.7.0 se conserva arriba para no borrar evidencias ni presentar sus checks como nuevos.

- **24/24 tests locales aprobados**, sin skipped: recuperación/conflictos, grafo/ciclos, snapshots/secretos, registro SQLite, capacidades de agentes, cancelación, resultados bloqueados, fallidos, timeout y ausencia de pruebas.
- **1 integración real Docker aprobada** (`tests/integration/docker.test.cjs`), sin mocks. Dentro del contenedor pasaron dos pruebas: parche e aislamiento. Se comprobó usuario 1000, ausencia de `.env`, imposibilidad de escritura en workspace y `/etc`, e interfaces de red internas solamente.
- La misma integración comprobó el original intacto antes de apply, rechazo antes de tener un recibo, invalidación por otro archivo cambiado, apply verificado, rollback exacto, recibo fallido y rechazo de apply tras tests fallidos.
- Imagen de esa ejecución: `node@sha256:116b800c3754b440ee0aa3ed2f20f5bf3d99a554bfd9adeea09ea8017f6e85f5`. La integración completa tomó aproximadamente 28 segundos en la primera corrida y 62 segundos en la repetición bajo carga; ese tiempo no es un presupuesto garantizado de producto.
- Benchmark de grafo reproducido en Apple M2 / 8 GB / Node 24.13.1 con fixtures de 100/1000/10000 archivos y tres muestras en proceso caliente. Resultados en `benchmarks/graph-local.json`. El caso de 10000 analiza hasta 1000 módulos e informa truncado; no demuestra indexación semántica completa de 10000 módulos.
- Build browser/node/electron y typecheck sin errores. Arranque de desarrollo llegó a ready sin errores de inyección; se observó el nuevo panel 0.7.1 con verificación. El Mac volvió a bloquearse y no se completó la interacción visual de proponer/aplicar/revertir.
- Se corrigieron los dos `uriScheme` generados a `forgeide`, coherentes con el protocolo del instalador. El login OAuth completo sigue sin comprobarse.
- La auditoría archivada de dependencias sigue pendiente de resolución; no se hicieron cambios ciegos de versiones mayores ni se declara eliminado ese riesgo.

Docker inicialmente no pudo arrancar por falta de espacio. Se retiraron caches temporales de compilación de este proyecto, se reinició Docker sin borrar imágenes ni volúmenes del usuario y se descargó la imagen de pruebas. Después se ejecutó la integración real anterior. El arranque fallido o la descarga fallida no se contaron como pruebas aprobadas.

## Paquete 0.7.1 comprobado

- `yarn package:mac` completó el paquete arm64. La compilación completa y typecheck fueron correctos.
- `scripts/check-packaged-pty.cjs` ejecutado con el Electron distribuido: PASS.
- La integración Docker se repitió usando `ELECTRON_RUN_AS_NODE=1` y el binario distribuido: PASS, aproximadamente 55 segundos. Es la misma integración repetida en otro runtime; no se cuenta como otro caso de prueba distinto.
- El `.app` distribuido inició backend en loopback, sincronizó 90 plugins y llegó a ready sin errores en ese arranque. No sustituye las pruebas interactivas de Git/login o depuración.
- La compilación del paquete incluye el módulo Forge dentro de los bundles; las pruebas del servicio importan su código compilado y el runtime de Electron. No se presenta esa integración como un E2E de clics de la interfaz.
