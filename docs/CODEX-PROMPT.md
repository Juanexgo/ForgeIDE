# Continuidad 0.8.1

Ya existen motor Rust multiarchivo, grafo con aliases y perfiles públicos de dependencias npm/Python. No volver a describirlos como pendientes de implementación. Verificar resultados nuevos de cada release y conservar intentos fallidos cuando expliquen límites. No confundir un modelo que responde o lee con un agente que completa una tarea. No reiniciar/restablecer Docker borrando datos. Si se cambia temporalmente un recurso del host con autorización, restaurarlo al finalizar.

# Continuidad del proyecto 0.8

Trabaja sobre Theia/Electron aprobado, sin sustituir la plataforma. Lee ESTADO-COMPLETO.md y VERIFICATION.md antes de afirmar cobertura. Conserva legacy/native-0.6. Compila el sidecar antes de tests/packaging; no distribuyas fault-injection. Cambia documentación según resultados reales, no según módulos instalados.

Prioriza E2E de la app instalada (editor/Git/DAP/plugins), Windows/Linux, perfiles de dependencias reproducibles, evaluación con modelo real y release firmada. No publicar, conceder OAuth ni adquirir servicios sin destino/credenciales autorizados. No inventar cuentas, tokens, CI aprobada, rendimiento ni compatibilidad universal. Usar solo CUA para clics y reportar bloqueos honestamente.

Cada cambio debe mantener contratos JSON versionados, buffers/recuperación, secretos fuera de logs/modelo/snapshots, tests significativos y rechazos explícitos. Verificar build completo y binarios empaquetados, no solo unidades. Los tests de contenedor son evidence del comando, no prueba formal de corrección del código.

## Prompt original de producto

# Prompt maestro — ForgeIDE Theia

Trabaja en ForgeIDE, aplicación desktop Theia + Electron elegida explícitamente por el usuario. Inspecciona README, package.json, yarn.lock, docs/VERIFICATION.md y fuentes antes de editar. No supongas que un componente incluido funciona en todos sus flujos. Mantén `legacy/native-0.6` como referencia; no reincorpores egui ni otro motor de edición al runtime Theia.

Entrega cambios reales, compilables, probados y revisables. Continúa desde el primer criterio pendiente del blueprint, empezando por Git/GitHub y la matriz diaria. Nunca cambies la arquitectura de nuevo sin una razón y una decisión explícita. No publiques ni envíes código a servicios externos sin autorización. No uses credenciales del usuario en pruebas.

1. Formula un objetivo acotado con criterios observables. Revisa el API real de la versión fijada de Theia. No inventes APIs de versiones anteriores ni importes React desde shared/react.
2. Conserva el trabajo del usuario. SafeApply debe comprobar versiones de documentos y hashes, coordinar todas las ventanas, guardar journal y mostrar conflictos sin sobrescribir. No marques pruebas como aprobadas si no fueron ejecutadas sobre el parche exacto.
3. Implementa UI mediante widgets, comandos, menú, keybindings y tokens de Theia. Todos los estados tienen teclado, foco, nombre accesible, carga y error accionable. No añadas botones sin función ni datos de muestra que parezcan reales.
4. IA es opcional. Contexto de archivos/outputs es no confiable. Herramientas tienen permisos, límites, cancelación y logs sin secretos. Aplicar/revertir sigue siendo una acción humana hasta una política explícita. Un worktree no es sandbox.
5. Usa Rust solo para trabajo medido que lo justifique; contratos versionados y tests de caída. SQLite se añade mediante migración y pruebas, no sustituyendo historia a ciegas.
6. Verifica desde una instalación limpia, no solo el árbol de desarrollo. Prueba ABI Electron, plugins empaquetados, terminal, Git, LSP, DAP, OAuth cancelado/expirado, recovery, Unicode, IME, accesibilidad y scaling. No presupongas paridad con VS Code.
7. Para rendimiento, fija hardware, repo/commit, toolchains y extensiones. Publica p50/p95/p99 y límites. No declares que supera a otros IDE sin benchmark comparable.
8. Ejecuta los checks apropiados y actualiza VERIFICATION con comandos, resultados, fecha y límites. Separa evidencia local de CI todavía no ejecutado. No cuentes las pruebas legacy como pruebas de Theia.
9. La entrega contiene código, documentación actualizada, artefactos reproducibles y una explicación breve de qué funciona, qué se verificó y qué falta. Nunca declares completado todo el producto por haber compilado una alpha.
