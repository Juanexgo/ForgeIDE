# ForgeIDE 0.9.0-alpha.1

IDE desktop Theia 1.77.0 + Electron 44.9.1, TypeScript/React y un motor transaccional Rust. La versión nativa previa se conserva en legacy/native-0.6. Esta entrega es alpha: no certifica paridad universal con VS Code ni superioridad sobre otros IDE.

## Construcción

Requisitos: Node 24, Yarn 1.22.22, Rust 1.85.1+, Git, Python 3 y herramientas nativas de compilación del SO. Docker Desktop/Engine es opcional para las pruebas aisladas; GitHub CLI (`gh`) es necesario para la cuenta y los PR. No se incluyen credenciales. Reservar varios GB de disco.

```sh
corepack enable
yarn install --frozen-lockfile
yarn download:plugins
yarn rebuild
yarn build
yarn run typecheck
yarn test
yarn start
```

`yarn rebuild` prepara módulos nativos para Electron. `yarn build` compila el sidecar Rust y los bundles. El lanzador usa loopback. No publicar este backend en la red.

## Usar Forge

Cierra la versión anterior antes de abrir un paquete nuevo: una instancia ya abierta puede recibir el lanzamiento e impedir ver las mejoras. Guarda tu trabajo primero. La nueva interfaz muestra 0.9.0.

- Cmd/Ctrl+Alt+F abre Forge Control: propuestas, grafo, diagnóstico y herramientas de lenguaje.
- Cmd/Ctrl+Alt+G abre Git: archivos, fragmentos, commits, ramas, comparación, conflictos, remotos, worktrees y PR.
- Conectar GitHub abre el flujo web de `gh`; la persona aprueba en su navegador. Forge muestra el código temporal, nunca el token. La identidad local de commits se configura por separado. Las operaciones de escritura requieren marcar la confianza del workspace durante la sesión.
- En propuestas multiarchivo, revisar cada diff y las eliminaciones. Puede crearse una nueva propuesta con archivos seleccionados. Aplicar sin pruebas es manual y no implica verificación.
- Preparar runtime permite elegir Node o Python. Los perfiles opcionales de dependencias públicas preparan un entorno separado mediante un plan con consentimiento; ver docs/DEPENDENCY-PROFILES.md. La verificación ejecuta `node --test` o `python -m unittest discover -v` sobre una copia sin secretos conocidos, sin red ni dependencias externas del host. El arranque del contenedor tiene un presupuesto separado de 120 segundos; los tests conservan 60 segundos y ambos tiempos quedan en el recibo. El resultado queda ligado al parche y al snapshot; cualquier cambio posterior de inputs exige repetir pruebas.
- Revertir exige que el contenido todavía corresponda al aplicado. La recuperación conserva cambios externos y comunica conflictos.
- Open VSX permite instalar extensiones compatibles. El host Node tiene privilegios del usuario: no es un sandbox. Los proveedores de IA se configuran aparte; no se distribuyen claves.

## Comprobaciones adicionales

```sh
docker pull node:24-alpine
docker pull python:3.13-alpine
yarn test:docker
cargo build --locked --manifest-path engine/Cargo.toml --features fault-injection
# FORGE_ENGINE_FAULT_PATH apunta a engine/target/debug/forge-engine (o .exe)
yarn test:crash
node scripts/benchmark.mjs
yarn package:mac
```

El paquete macOS se escribe en dist-0.9/mac-arm64. Windows/Linux disponen de workflows, pendientes de ejecución real. La alpha carece de firma de distribución de Apple/Microsoft y notarización. Incluye comprobación de un feed Ed25519 y descarga manual verificada; no ejecuta actualizaciones automáticamente. El feed debe publicarse antes de que la comprobación remota tenga una versión disponible.

## Documentación

- [Perfiles de dependencias](docs/DEPENDENCY-PROFILES.md)
- [Estado real por sistema](docs/ESTADO-COMPLETO.md)
- [Pruebas y evidencia](docs/VERIFICATION.md)
- [Arquitectura y PRD](docs/BLUEPRINT.md)
- [Contratos y seguridad](docs/SECURITY-AND-API.md)
- [Prompt maestro](docs/CODEX-PROMPT.md)
- [Auditoría de dependencias](docs/dependency-audit.json)

Solo se han validado localmente los flujos expresamente descritos en la evidencia; la integración de un módulo Theia no equivale a certificar todos sus comportamientos.

## Cambios 0.9 en validación

Graph incremental añade análisis estático Python, Go, Rust y servicios Compose. Las propuestas de agentes tienen límites de tiempo y herramientas y una alternativa JSON revisable. GitHub permite desconectar ForgeIDE sin borrar credenciales compartidas. Pruebas reales de protocolo TypeScript y DAP complementan las pruebas locales; no sustituyen la validación visual ni de accesibilidad. Consulte docs/ESTADO-COMPLETO.md para límites y evidencia, sin asumir certificación multiplataforma.
