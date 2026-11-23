# Firma de distribución

El usuario confirmó que no dispone de certificados Developer ID ni de firma Windows. Los instaladores alpha continúan sin esas firmas. Una firma Ed25519 del feed de actualización verifica metadatos de ForgeIDE; no equivale a Developer ID, notarización ni Authenticode.

La configuración `apps/desktop/electron-builder.signed.cjs` y el workflow manual `signed-distribution.yml` rechazan la ausencia de credenciales. Nunca degradan silenciosamente una compilación firmada a unsigned ni aceptan Apple Development como identidad de distribución.

Preparar el entorno GitHub `distribution-signing`, con revisores y acceso restringido a la rama de release. Configurar estos secretos por los canales privados de GitHub, nunca en el código ni en el chat:

- Apple: `MAC_CSC_LINK`, `MAC_CSC_KEY_PASSWORD`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID`; variable `MAC_SIGNING_IDENTITY` con identidad Developer ID Application.
- Windows: `WIN_CSC_LINK`, `WIN_CSC_KEY_PASSWORD`; variable `WIN_PUBLISHER_NAME` correspondiente al certificado.

Los enlaces CSC deben apuntar a certificados de distribución válidos o contener su formato codificado compatible con electron-builder. Una clave no exportable/HSM exige adaptar la integración al proveedor; no se afirma compatibilidad ya validada con cualquier proveedor de firma.

Al ejecutar el workflow, cada sistema construye, prueba y firma en su runner nativo. macOS debe superar `codesign --verify`, Gatekeeper y la validación del ticket notarizado. Windows exige Authenticode válido tanto en la aplicación como en el instalador. Los artefactos se publican únicamente después de esas verificaciones. Falta ejecutar el flujo con certificados reales y validar instalación/actualización en equipos limpios.
