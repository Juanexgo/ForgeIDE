# Licencias de terceros

El código nuevo de ForgeIDE se ofrece bajo MIT, ver LICENSE. La aplicación distribuida incluye software con licencias distintas. Este documento es un índice de revisión, no sustituye los textos originales.

- Eclipse Theia: EPL-2.0 / licencia secundaria indicada en cada archivo. Fuente: https://github.com/eclipse-theia/theia
- Electron: MIT y licencias de Chromium/Node/terceros incluidas en su distribución.
- Monaco: MIT y avisos en sus paquetes.
- React, TypeScript y demás paquetes: consultar LICENSE y metadatos incluidos en node_modules.
- Builtins VS Code: cada extensión incluye sus avisos y licencia (`LICENSE-vscode.txt` cuando corresponde). Fuente de compilación: https://github.com/eclipse-theia/vscode-builtin-extensions
- El renderer Electron usa la sustitución de ffmpeg sin codecs propietarios de Theia durante build. El empaquetador debe usar ese mismo runtime preparado.

Antes de una distribución pública comercial o comunitaria: generar SBOM, inventario completo de licencias, conservar avisos, revisar obligaciones y compatibilidad de cada extensión. No se ha realizado una revisión legal completa ni publicado el producto.

## Inventario 0.8

`node scripts/inventory.mjs` genera docs/sbom-lockfile.cdx.json, inventario plano CycloneDX del lockfile NPM, incluyendo desarrollo. No representa todavía todos los binarios/plugins/Cargo ni relaciones de dependencias o licencias verificadas. La revisión completa sigue pendiente. El sidecar Rust usa fs2, tempfile, serde/serde_json y unicode-normalization; sus versiones exactas están en engine/Cargo.lock.
