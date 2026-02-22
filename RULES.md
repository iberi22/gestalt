# 🤖 RULES.md - Reglas para Agentes de IA

> Reglas obligatorias para cualquier agente de IA que trabaje en este proyecto.

_Última actualización: 2025-12-19_

---

## 🔄 Conciencia del Proyecto y Contexto

### Al iniciar cualquier conversación:
1. **Siempre leer `.gitcore/ARCHITECTURE.md`** para entender la arquitectura, objetivos y restricciones del proyecto.
2. **Consultar `.gitcore/planning/TASK.md` o GitHub Issues** antes de comenzar cualquier trabajo. Si la tarea no está listada, añadirla con descripción breve y fecha.
3. **Usar convenciones de nombres, estructura de archivos y patrones de arquitectura** descritos en `.gitcore/ARCHITECTURE.md`.

### Línea de Tiempo:
4. **Registrar timestamp en todas las operaciones**. El tiempo es la variable primaria del sistema.
5. **Usar siempre UTC** para timestamps internos.
6. **Consultar la timeline antes de actuar** para entender el contexto reciente.

---

## 🧱 Estructura de Código y Modularidad

### Archivos:
7. **Nunca crear archivos con más de 800 líneas de código.** Si se acerca al límite, refactorizar en módulos.
8. **Organizar código en módulos claramente separados** por feature o responsabilidad.
9. **Usar imports claros y consistentes** (preferir imports relativos dentro de crates).

### Rust-específico:
10. **Seguir las convenciones de Rust**: snake_case para funciones/variables, CamelCase para tipos.
11. **Usar `Result<T, E>` para manejo de errores**, no `unwrap()` en código de producción.
12. **Aprovechar el sistema de tipos de Rust** para prevenir errores en compilación.

---

## 🧪 Testing y Confiabilidad

### Tests obligatorios:
13. **Crear tests unitarios para nuevas funcionalidades** (funciones, structs, comandos CLI).
14. **Después de actualizar lógica**, verificar si los tests existentes necesitan actualizarse.
15. **Los tests deben vivir en `/tests` o como módulos `#[cfg(test)]`**.

### Cobertura mínima:
16. Incluir al menos:
    - 1 test para uso esperado (happy path)
    - 1 caso edge
    - 1 caso de fallo/error

---

## ✅ Completar Tareas

17. **Marcar tareas completadas en `.gitcore/planning/TASK.md` o Issues** inmediatamente después de terminarlas.
18. **Añadir nuevas sub-tareas o TODOs descubiertos** bajo "Tareas Descubiertas" en los archivos de planificación correspondientes en `.gitcore/planning/`.
19. **Actualizar `CHANGELOG.md`** cuando se complete una feature significativa.

---

## 📎 Estilo y Convenciones

### Lenguaje y herramientas:
20. **Usar Rust** como lenguaje principal.
21. **Formatear con `cargo fmt`** antes de commit.
22. **Verificar con `cargo clippy`** para lints.
23. **Usar `tokio` para async**, `serde` para serialización, `clap` para CLI.

### Documentación en código:
24. **Escribir doc comments para funciones públicas**:
```rust
/// Brief summary of what this function does.
///
/// # Arguments
///
/// * `param1` - Description of param1
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// When and why this function returns an error
pub fn example(param1: &str) -> Result<String, Error> {
    // ...
}
```

---

## 📚 Documentación y Explicabilidad

25. **Actualizar `README.md`** cuando se añadan features, cambien dependencias, o se modifiquen pasos de setup.
26. **Comentar código no obvio** y asegurar que todo sea entendible para un desarrollador de nivel medio.
27. **Añadir comentarios `// Reason:` inline** explicando el porqué de lógica compleja, no solo el qué.

---

## 🧠 Reglas de Comportamiento de IA

28. **Nunca asumir contexto faltante. Preguntar si hay dudas.**
29. **Nunca inventar librerías o funciones** – solo usar crates verificados de crates.io.
30. **Siempre confirmar paths y nombres de módulos** existen antes de referenciarlos.
31. **Nunca eliminar o sobrescribir código existente** a menos que sea explícitamente instruido o parte de una tarea en `TASK.md`.

---

## 🕐 Timeline-Específico

32. **Cada comando CLI debe registrar un evento en la timeline**.
33. **Incluir `agent_id` en todos los eventos** para trazabilidad.
34. **Los errores también se registran** con `EventType::TaskFailed` o similar.
35. **Consultar timeline con `--since` apropiado** para contexto sin sobrecargar.

---

## 🔗 Referencias Rápidas

| Documento | Propósito |
|-----------|-----------|
| `.gitcore/ARCHITECTURE.md` | Arquitectura y decisiones técnicas |
| `.gitcore/planning/TASK.md` | Estado actual (Histórico) / Issues (Activo) |
| `README.md` | Guía de uso y roadmap |
| `CHANGELOG.md` | Historial de cambios |
| `RULES.md` | Este archivo |
| `AGENTS.md` | Configuración de Agentes |

---

## 📋 Checklist Pre-Commit

Antes de hacer commit, verificar:

- [ ] `cargo fmt` ejecutado
- [ ] `cargo clippy` sin warnings
- [ ] `cargo test` pasa
- [ ] `TASK.md` actualizado si corresponde
- [ ] `CHANGELOG.md` actualizado para features
- [ ] Doc comments en funciones públicas nuevas
