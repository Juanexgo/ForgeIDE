# Empaquetado de ForgeIDE

## macOS

```sh
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked -p forge-desktop
packaging/macos/package.sh target/release/forgeide ForgeIDE-macOS.zip
packaging/macos/disk-image.sh ForgeIDE-macOS.zip ForgeIDE-macOS.dmg
```

El DMG incluye la aplicación con firma ad hoc y un enlace a Applications. La distribución pública necesita una identidad Developer ID Application bajo control del editor y notarización de Apple.

## Ubuntu 24.04

```sh
cargo build --release --locked -p forge-desktop
packaging/linux/package.sh target/release/forgeide ForgeIDE-Linux.deb
dpkg-deb --info ForgeIDE-Linux.deb
```

El script utiliza la arquitectura del sistema de compilación. Usa el artefacto correspondiente al procesador de destino. La distribución pública mediante repositorio APT requiere firmar su índice.

## Windows x64

El flujo de CI compila y prueba la variante MSVC en un runner de Windows. Para crear un MSI de desarrollo con un compilador compatible con WiX 3, coloca `forgeide.exe`, `packaging/windows/ForgeIDE.wxs` y los dos archivos `*-OFL.txt` de `crates/forge-desktop/assets` en el mismo directorio. Con wixl:

```sh
wixl --arch x64 -D Binary=forgeide.exe -o ForgeIDE-Windows-x64.msi ForgeIDE.wxs
msiextract -l ForgeIDE-Windows-x64.msi
```

El MSI instala ForgeIDE bajo Program Files. La distribución pública requiere firma Authenticode, además de ejecutar el ciclo de instalación, arranque, actualización y desinstalación en Windows.

Consulta [la matriz de verificación](VERIFICATION.md) para conocer los paquetes y flujos comprobados.
