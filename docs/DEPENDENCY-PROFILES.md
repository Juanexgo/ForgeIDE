# Perfiles de dependencias — 0.8.1

El objetivo es ejecutar pruebas con bibliotecas externas sin montar credenciales ni node_modules del host. La preparación usa Docker con red; la ejecución posterior sigue aislada sin red, readonly y usuario 1000. No son el mismo entorno ni las mismas garantías.

1. Crear una propuesta multiarchivo y preparar el runtime oficial Node/Python.
2. Revisar el plan de dependencias y su digest. Se calcula sobre los manifiestos propuestos, no solo los actuales.
3. Confirmar la descarga pública. Solo los manifiestos validados y un Dockerfile generado se envían al builder local. Se excluyen código fuente, npmrc, dotfiles, credenciales y Dockerfiles del proyecto.
4. Construir una imagen y guardar su ID SHA-256 junto al perfil, base inmutable y digest del plan. No se permite usar un ID arbitrario no registrado.
5. Las pruebas se ejecutan en esa imagen sin red. Resolver un perfil obsoleto se rechaza antes de ejecutar código. El recibo incluye dependencyProof y workdir, además del snapshot del código.
6. Aplicar con verificación exige el último recibo aprobado y los mismos inputs; revertir conserva los controles de conflicto del motor Rust.

## Soporte inicial

- npm: package-lock v2/v3, un único paquete, paquetes públicos de registry.npmjs.org con integridad SHA-256/384/512. npm ci ignora scripts y no hereda npmrc. No hay workspaces, enlaces, Git/file/URL dependencies, mirrors ni autenticación privada. Máximo 512 paquetes bloqueados.
- Python: requirements.txt con versiones exactas y hashes SHA-256. Solo wheels, sin source builds; no instala dependencias implícitas: todas las transitivas necesarias deben figurar explícitamente y llevar hash. Sin URLs, opciones de índices, markers ni extras.
- La imagen base e integridades fijadas ayudan a reproducir inputs; no certifican que un paquete sea benigno. Paquetes que requieren lifecycle scripts, extensiones nativas o servicios pueden fallar, y eso no debe convertirse en verde.
- Node usa /opt/forge/deps/project como cwd para resolver dependencias desde su directorio padre; Python usa /workspace y PYTHONPATH del runtime preparado.
- El Dockerfile y los comandos no se toman del repositorio. Docker BuildKit usa red para descargar; no se promete un firewall de destinos del builder. Los registros pueden redirigir descargas. No se pasa código ni claves del proyecto.
- La preparación está limitada a 180s por el cliente. Cancelar desconecta el cliente de BuildKit; su caché puede permanecer. Las imágenes/cachés propias ocupan espacio y su política de retención queda pendiente. No se podan imágenes/volúmenes ajenos.
- Los registros locales son un límite de integridad cooperante, no un mecanismo contra un administrador del host o un usuario que modifique el estado de Docker.

La opción de verificar sin dependencias sigue disponible con el runtime oficial. Un fallo al preparar no crea recibo aprobado.

## Referencias de comportamiento

La instalación se basa en [npm ci](https://docs.npmjs.com/cli/v11/commands/npm-ci/) con scripts deshabilitados y en el [modo de hashes y wheels de pip](https://pip.pypa.io/en/stable/topics/secure-installs/). El código aplica restricciones adicionales y no presenta esos mecanismos como aislamiento absoluto.

## Presupuestos de ejecución

El runner permite hasta 120s para iniciar el contenedor controlado, y después mide 60s para los tests mediante docker exec. Si el arranque falla no hay verificación aprobada. El contenedor conserva red deshabilitada, readonly, caps eliminadas, usuario 1000 y límites de recursos. Un proceso fijo sleep 240 mantiene vivo el contenedor, que se elimina al finalizar/cancelar. startupMs y executionMs separan ambos tiempos en el recibo.
