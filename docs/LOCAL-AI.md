# IA local reproducible

Perfil de aceptación del 23 de noviembre de 2026: Ollama con `qwen3:4b-instruct-2507-q4_K_M` (aproximadamente 2.5 GB de descarga). Los cuatro roles utilizan las mismas definiciones de herramientas que el panel y el backend. El modelo Thinking `qwen3:4b` no es intercambiable con este perfil: los ensayos anteriores agotaban el presupuesto razonando incluso al solicitar una respuesta sin razonamiento.

1. Instalar Ollama desde su distribución oficial e iniciar el servicio local.
2. Ejecutar `ollama pull qwen3:4b-instruct-2507-q4_K_M`.
3. Activar las capacidades IA y configurar el proveedor Ollama en ForgeIDE. Estas son preferencias reales de Theia 1.77; se pueden copiar al archivo de configuración de usuario:

```json
{
  "ai-features.AiEnable.enableAI": true,
  "ai-features.ollama.ollamaHost": "http://127.0.0.1:11434",
  "ai-features.ollama.ollamaModels": ["qwen3:4b-instruct-2507-q4_K_M"],
  "ai-features.languageModelAliases": {
    "default/code": {"selectedModel": "ollama/qwen3:4b-instruct-2507-q4_K_M"}
  }
}
```

Seleccionar ForgeImplement, ForgeFix, ForgeTests o ForgeSecurity en el chat. Pedir una tarea pequeña y revisable sobre archivos concretos. Una propuesta persistida termina el turno y muestra su identificador real. El resultado sigue pendiente de revisión y verificación; no se presenta como una tarea aplicada o aprobada. Revisar el diff en Forge Control, verificar con el perfil aislado correspondiente y aplicar solo después de aprobarlo. La reversión conserva las comprobaciones de baseline para no sobrescribir cambios posteriores.

Los controles de generación permitidos están limitados a contexto, presupuesto de salida, temperatura y muestreo. No permiten introducir herramientas, servidores o comandos arbitrarios. Al terminar un turno Forge se descarga de la memoria el modelo de un proveedor loopback si no queda otra solicitud activa de la aplicación; los pesos descargados se conservan. Otros clientes de Ollama no comparten el contador de actividad, por lo que pueden volver a cargar el modelo si lo necesitan.

## Repetir la aceptación

Con el motor Rust y la extensión compilados, Node 24 y Docker local activo:

```sh
FORGE_OLLAMA_HOST=http://127.0.0.1:11434 FORGE_OLLAMA_MODEL=qwen3:4b-instruct-2507-q4_K_M yarn test:agents
```

En un equipo con poca memoria se pueden separar las fases: detener Docker antes de empezar y usar `FORGE_START_DOCKER=1`; el ensayo genera las cuatro propuestas antes de iniciar Docker. Este reinicio afecta contenedores existentes, por lo que requiere haber reservado el equipo para la prueba. No cambiar la memoria de Docker sin autorización.

El ensayo exige lectura real de archivos, una propuesta persistida, ausencia de escrituras durante generación, verificación real sin red, aplicación y reversión exacta. ForgeTests además debe detectar una mutación introducida deliberadamente. Produce `benchmarks/agents-real-matrix.json`. Son cuatro fixtures sintéticos; no sustituyen la validación del chat gráfico ni una evaluación general de calidad del modelo.
