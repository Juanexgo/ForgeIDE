# Distribución y firma

## Paquetes alpha

`Package preview (unsigned)` compila en el sistema de destino, ejecuta las pruebas y genera ZIP macOS, NSIS Windows, AppImage y DEB Linux. Las bibliotecas nativas se reconstruyen para el Electron distribuido. Un artefacto de Actions demuestra creación del paquete, no prueba de instalación gráfica ni aprobación del sistema operativo.

Para repetir localmente, después del build: `yarn workspace @forgeide/desktop run package:preview:mac` (o `:win`, `:linux`). El directorio es `dist-0.9`.

## Certificados de distribución pendientes

No se ha localizado un certificado Developer ID Application ni un certificado de firma Windows. No usar Apple Development como sustituto de una firma de distribución. No generar un certificado autofirmado para presentarlo como firma comercial reconocida.

Al disponer de certificados, configurar un entorno de publicación protegido en GitHub y suministrar CSC_LINK/CSC_KEY_PASSWORD a electron-builder. En macOS eliminar el `mac.identity: null` de la configuración **solo para esa ejecución**, seleccionar la identidad Developer ID esperada, habilitar hardenedRuntime y notarización con las credenciales de la cuenta Apple. Después verificar con `codesign --verify --deep --strict`, `spctl --assess --type execute`, y `xcrun stapler validate`. En Windows verificar firma y cadena del instalador con el almacén de confianza del sistema. No incluir PFX, contraseñas, claves privadas ni credenciales Apple en el repositorio o logs.

Esos pasos requieren las identidades legítimas del titular y no quedan aprobados por la simple existencia de un workflow.

## Actualizaciones verificadas y manuales

La política pública está en apps/desktop/resources/update-policy.json. Se fija una clave Ed25519, el repositorio y el feed HTTPS. El backend verifica firma, caducidad, versión monotónica, origen, hash y tamaño. Descarga a un archivo privado temporal y solo lo renombra si todo coincide; nunca ejecuta el instalador.

El feed tiene un sobre `{payload,signature}`, con payload JSON codificado en base64 y firma de los bytes exactos. Generarlo con `node scripts/sign-update-feed.mjs manifest.json /ruta/privada/update-ed25519.pem update-feed.json`. La herramienta verifica su propia salida. Manifest: schema 1, versión semver, fecha de generación, caducidad máxima de 30 días y artefactos con plataforma, arquitectura, kind, URL de release del repositorio permitido, bytes y SHA256. Consultar UpdateArtifact y verifyFeed para los nombres exactos.

Publicar artefactos reales antes de publicar el feed. Actualizar el archivo en el release `updates-alpha` preservando la clave pública fijada. Conservar una copia protegida de la clave privada fuera del código y de los outputs; perderla exige una rotación mediante una entrega confiable con nueva política. La clave Ed25519 del feed no reemplaza notarización ni firma de Windows. Una actualización vencida o inaccesible se comunica como error, no como “al día”.
