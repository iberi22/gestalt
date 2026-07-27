//! 🧠 Agent Registry — Catálogo de capacidades, rate limits y modelos
//!
//! Mantiene un inventario de todos los agentes disponibles (locales y remotos),
//! sus capacidades, límites de tasa, y el enrutamiento tarea → agente.
//!
//! # Uso
//! ```rust,no_run,ignore
//! let mut registry = AgentRegistry::load("agent-registry.toml")?;
//! if let Some(best) = registry.select_agent("edit file", None) {
//!     println!("Best agent: {} (provider: {})", best.name, best.provider);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// Registry completo de agentes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistry {
    /// Agentes disponibles
    pub agents: Vec<AgentEntry>,
    /// Mapa de proveedores y sus límites globales
    pub providers: HashMap<String, ProviderConfig>,
    /// Estado de uso de los proveedores
    #[serde(default)]
    pub provider_states: HashMap<String, ProviderState>,
    /// Preferencias de enrutamiento
    #[serde(default)]
    pub routing: RoutingConfig,
}

/// Un agente en el inventario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    /// Nombre único (ej: "kimi", "agy", "hermes")
    pub name: String,
    /// Proveedor (ej: "openrouter", "local", "openclaw")
    pub provider: String,
    /// Modelo específico
    pub model: String,
    /// Tipo de agente
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    /// Capacidades que posee
    pub capabilities: Vec<String>,
    /// Rate limits por minuto
    #[serde(default)]
    pub rate_limit: RateLimit,
    /// Costo por millón de tokens (USD)
    #[serde(default)]
    pub cost_per_mtok: f64,
    /// Contexto máximo en tokens
    #[serde(default = "default_max_context")]
    pub max_context: u64,
    /// Estado actual
    #[serde(default)]
    pub status: AgentStatus,
    /// Etiquetas para enrutamiento
    #[serde(default)]
    pub tags: Vec<String>,
    /// Cuándo se marcó como ocupado (para timeout automático)
    #[serde(skip)]
    pub ocupado_desde: Option<Instant>,
}

fn default_max_context() -> u64 {
    4096
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentType {
    /// CLI local (agy, kimi, cursor-agent)
    Cli,
    /// API remota (OpenRouter, OpenClaw)
    Api,
    /// Modelo local diminuto (Phi, TinyLlama, Qwen2.5-Coder-0.5B)
    Tiny,
    /// Asistente conversacional (Hermes)
    Assistant,
    /// Orquestador (Gestalt Router)
    Orchestrator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Requests por minuto
    #[serde(default = "default_rpm")]
    pub rpm: u32,
    /// Tokens por minuto
    #[serde(default = "default_tpm")]
    pub tpm: u64,
    /// Requests en el minuto actual (reset cada 60s)
    #[serde(default)]
    pub current_rpm: u32,
    /// Tokens en el minuto actual
    #[serde(default)]
    pub current_tpm: u64,
}

fn default_rpm() -> u32 {
    60
}
fn default_tpm() -> u64 {
    100_000
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            rpm: default_rpm(),
            tpm: default_tpm(),
            current_rpm: 0,
            current_tpm: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Disponible,
    Ocupado,
    Error(String),
}

impl Default for AgentStatus {
    fn default() -> Self {
        AgentStatus::Disponible
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// URL base de la API
    pub base_url: String,
    /// Límites globales del proveedor
    pub rate_limit: RateLimit,
    /// Prioridad (menor = más prioritario)
    #[serde(default = "default_priority")]
    pub priority: u8,
}

/// Estado de uso de un proveedor
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderState {
    /// Requests en el minuto actual
    #[serde(default)]
    pub current_rpm: u32,
    /// Tokens en el minuto actual
    #[serde(default)]
    pub current_tpm: u64,
}

fn default_priority() -> u8 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Estrategia por defecto
    #[serde(default)]
    pub strategy: RoutingStrategy,
    /// Si true, prefiere modelos locales para tareas simples
    #[serde(default = "default_prefer_local_simple")]
    pub prefer_local_for_simple: bool,
    /// Si true, usa tiny agents para ediciones de una línea
    #[serde(default = "default_tiny_for_precise")]
    pub tiny_agents_for_precise_edits: bool,
}

fn default_prefer_local_simple() -> bool {
    true
}
fn default_tiny_for_precise() -> bool {
    true
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            strategy: RoutingStrategy::CapabilityMatch,
            prefer_local_for_simple: true,
            tiny_agents_for_precise_edits: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutingStrategy {
    /// Coincidencia por capacidades
    CapabilityMatch,
    /// Round-robin entre disponibles
    RoundRobin,
    /// Menor costo primero
    Cheapest,
    /// Mayor capacidad primero
    MostCapable,
}

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self::CapabilityMatch
    }
}

impl AgentRegistry {
    /// Carga el registry desde un archivo TOML
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| format!("No se pudo leer {}: {}", path.as_ref().display(), e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Error parseando {}: {}", path.as_ref().display(), e))
    }

    /// Carga el registry desde el contenido TOML directamente
    pub fn from_toml(content: &str) -> Result<Self, String> {
        toml::from_str(content).map_err(|e| format!("Error parseando TOML: {}", e))
    }

    /// Selecciona el mejor agente para una tarea
    pub fn select_agent(
        &mut self,
        task: &str,
        required_capability: Option<&str>,
    ) -> Option<&AgentEntry> {
        // Recuperar agentes que excedieron el timeout de Ocupado
        for agent in &mut self.agents {
            if agent.status == AgentStatus::Ocupado {
                if let Some(since) = agent.ocupado_desde {
                    if since.elapsed() > std::time::Duration::from_secs(60) {
                        agent.status = AgentStatus::Disponible;
                        agent.ocupado_desde = None;
                    }
                }
            }
        }

        let provider_states = &self.provider_states;
        let providers = &self.providers;

        let mut candidates: Vec<&AgentEntry> = self
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Disponible)
            .filter(|a| {
                if let Some(cap) = required_capability {
                    a.capabilities.iter().any(|c| c == cap)
                } else {
                    true
                }
            })
            .filter(|a| a.rate_limit.current_rpm < a.rate_limit.rpm)
            .filter(|a| a.rate_limit.current_tpm < a.rate_limit.tpm)
            .filter(|a| {
                // Check provider-level rate limits
                if let Some(state) = provider_states.get(&a.provider) {
                    if let Some(config) = providers.get(&a.provider) {
                        state.current_rpm < config.rate_limit.rpm
                            && state.current_tpm < config.rate_limit.tpm
                    } else {
                        true // No provider config, allow
                    }
                } else {
                    true // No provider state tracked, allow
                }
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Preferir tiny agents para tareas pequeñas con coincidencia por keyword
        if self.routing.tiny_agents_for_precise_edits && task.len() < 200 {
            let task_lower = task.to_lowercase();
            if let Some(tiny) = candidates.iter().find(|a| {
                a.agent_type == AgentType::Tiny
                    && ((task_lower.contains("insert")
                        && a.capabilities.iter().any(|c| c == "insert-line"))
                        || (task_lower.contains("delete")
                            && a.capabilities.iter().any(|c| c == "delete-line"))
                        || (task_lower.contains("replace")
                            && a.capabilities.iter().any(|c| c == "replace-line"))
                        || (task_lower.contains("search")
                            && a.capabilities.iter().any(|c| c == "semantic-search")))
            }) {
                return Some(tiny);
            }
        }

        // Aplicar estrategia de enrutamiento
        match self.routing.strategy {
            RoutingStrategy::Cheapest => {
                candidates.sort_by(|a, b| {
                    a.cost_per_mtok
                        .partial_cmp(&b.cost_per_mtok)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            },
            RoutingStrategy::MostCapable => {
                candidates.sort_by(|a, b| b.capabilities.len().cmp(&a.capabilities.len()));
            },
            RoutingStrategy::CapabilityMatch => {
                // Prefiere el que tenga más capacidades relevantes para la tarea
                let task_lower = task.to_lowercase();
                candidates.sort_by(|a, b| {
                    let a_score = a
                        .capabilities
                        .iter()
                        .filter(|c| task_lower.contains(&c.to_lowercase()))
                        .count();
                    let b_score = b
                        .capabilities
                        .iter()
                        .filter(|c| task_lower.contains(&c.to_lowercase()))
                        .count();
                    b_score.cmp(&a_score)
                });
            },
            RoutingStrategy::RoundRobin => {
                // Mantiene orden actual (rotación implícita)
            },
        }

        candidates.first().copied()
    }

    /// Marca un agente como ocupado
    pub fn mark_busy(&mut self, name: &str) {
        if let Some(agent) = self.agents.iter_mut().find(|a| a.name == name) {
            agent.status = AgentStatus::Ocupado;
            agent.ocupado_desde = Some(Instant::now());
        }
    }

    /// Marca un agente como disponible
    pub fn mark_available(&mut self, name: &str) {
        if let Some(agent) = self.agents.iter_mut().find(|a| a.name == name) {
            agent.status = AgentStatus::Disponible;
            agent.ocupado_desde = None;
        }
    }

    /// Registra uso de rate limit
    pub fn record_usage(&mut self, name: &str, tokens: u64) {
        let provider = self
            .agents
            .iter_mut()
            .find(|a| a.name == name)
            .map(|agent| {
                agent.rate_limit.current_rpm += 1;
                agent.rate_limit.current_tpm += tokens;
                agent.provider.clone()
            });

        // También actualizar el provider state
        if let Some(provider_name) = provider {
            let state = self.provider_states.entry(provider_name).or_default();
            state.current_rpm += 1;
            state.current_tpm += tokens;
        }
    }

    /// Resetea contadores de rate limit (llamar cada 60s)
    pub fn reset_rate_limits(&mut self) {
        for agent in &mut self.agents {
            agent.rate_limit.current_rpm = 0;
            agent.rate_limit.current_tpm = 0;
        }
        // También resetear provider states
        for state in self.provider_states.values_mut() {
            state.current_rpm = 0;
            state.current_tpm = 0;
        }
    }

    /// Lista todos los agentes con su estado
    pub fn list_agents(&self) -> Vec<&AgentEntry> {
        self.agents.iter().collect()
    }

    /// Busca agentes por capacidad (coincidencia exacta)
    pub fn find_by_capability(&self, capability: &str) -> Vec<&AgentEntry> {
        self.agents
            .iter()
            .filter(|a| a.capabilities.iter().any(|c| c == capability))
            .collect()
    }

    /// Agentes que pueden editar archivos
    pub fn editors(&self) -> Vec<&AgentEntry> {
        self.find_by_capability("edit")
    }

    /// Agentes locales (tiny + CLI)
    pub fn local_agents(&self) -> Vec<&AgentEntry> {
        self.agents
            .iter()
            .filter(|a| matches!(a.agent_type, AgentType::Tiny | AgentType::Cli))
            .collect()
    }

    /// Resumen de capacidades del registry
    pub fn summary(&self) -> AgentSummary {
        let total = self.agents.len();
        let available = self
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Disponible)
            .count();
        let tiny = self
            .agents
            .iter()
            .filter(|a| a.agent_type == AgentType::Tiny)
            .count();
        let editors = self.editors().len();
        let all_caps: Vec<String> = self
            .agents
            .iter()
            .flat_map(|a| a.capabilities.iter().map(|c| c.to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        AgentSummary {
            total,
            available,
            tiny,
            editors,
            unique_capabilities: all_caps,
        }
    }
}

/// Resumen del registry
#[derive(Debug, Clone, Serialize)]
pub struct AgentSummary {
    pub total: usize,
    pub available: usize,
    pub tiny: usize,
    pub editors: usize,
    pub unique_capabilities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_registry() {
        let toml = r#"
[[agents]]
name = "kimi"
provider = "local"
model = "kimi-v1"
type = "Cli"
capabilities = ["edit", "search", "analyze"]

[[agents]]
name = "tiny-editor"
provider = "local"
model = "phi-3-mini"
type = "Tiny"
capabilities = ["edit-single-line"]

[providers.local]
base_url = "http://localhost:11434"
rate_limit = { rpm = 60, tpm = 100000 }
"#;
        let registry = AgentRegistry::from_toml(toml).unwrap();
        assert_eq!(registry.agents.len(), 2);
        assert_eq!(registry.agents[0].name, "kimi");
        assert_eq!(registry.agents[1].agent_type, AgentType::Tiny);
    }

    #[test]
    fn test_select_agent_by_capability() {
        let toml = r#"
[[agents]]
name = "agy"
provider = "openrouter"
model = "gemini-2.0-flash"
type = "Cli"
capabilities = ["edit", "search", "analyze", "code-review"]

[[agents]]
name = "tiny-editor"
provider = "local"
model = "phi-3-mini"
type = "Tiny"
capabilities = ["edit-single-line", "insert-line"]

[[agents]]
name = "kimi"
provider = "local"
model = "kimi-v1"
type = "Cli"
capabilities = ["edit", "search", "reason"]

[providers.local]
base_url = "http://localhost:11434"
rate_limit = { rpm = 60, tpm = 100000 }
"#;
        let mut registry = AgentRegistry::from_toml(toml).unwrap();

        // Buscar agente con capacidad "edit"
        let agent = registry.select_agent("edit this file", Some("edit"));
        assert!(agent.is_some());
        assert!(agent.unwrap().capabilities.contains(&"edit".to_string()));

        // Tarea pequeña con keyword "insert" → tiny agent con "insert-line"
        let agent = registry.select_agent("insert line", None);
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().agent_type, AgentType::Tiny);

        // Buscar por capacidad específica
        let editors = registry.editors();
        assert_eq!(editors.len(), 2); // agy + kimi = 2 (tiny-editor has "edit-single-line" not "edit")
    }

    #[test]
    fn test_rate_limit_tracking() {
        let toml = r#"
[[agents]]
name = "test-agent"
provider = "local"
model = "test-v1"
type = "Cli"
capabilities = ["edit"]
rate_limit = { rpm = 2, tpm = 1000 }

[providers.local]
base_url = "http://localhost:11434"
rate_limit = { rpm = 60, tpm = 100000 }
"#;
        let mut registry = AgentRegistry::from_toml(toml).unwrap();

        // Primer uso
        assert!(registry.select_agent("task", None).is_some());
        registry.record_usage("test-agent", 100);
        assert_eq!(registry.agents[0].rate_limit.current_rpm, 1);

        // Segundo uso
        assert!(registry.select_agent("task", None).is_some());
        registry.record_usage("test-agent", 200);
        assert_eq!(registry.agents[0].rate_limit.current_rpm, 2);

        // Tercer uso — excede RPM
        assert!(registry.select_agent("task", None).is_none());

        // Reset
        registry.reset_rate_limits();
        assert!(registry.select_agent("task", None).is_some());
    }
}
