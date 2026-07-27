use serde::{Deserialize, Serialize};

/// Supported front-end interactive components for generative user interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetType {
    Button,
    Markdown,
    CodeBlock,
    Input,
    Card,
}

/// A structured visual or interactive component dynamic user interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiElement {
    /// The specific type of the interactive widget.
    #[serde(rename = "type")]
    pub widget_type: WidgetType,
    /// JSON encoded layout/properties or simple text depending on widget specifications.
    pub content: String,
    /// Unique identifier for client-side state-binding and events.
    pub id: String,
}

/// Response returned from the orchestration hub containing both conversational text and structural visual assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerResponse {
    /// Natural language explanation or summary from the synthesized responses.
    pub text_response: String,
    /// An optional Dynamic GenUI component to render side-by-side.
    pub ui_component: Option<UiElement>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_element_serialization_roundtrip() {
        let element = UiElement {
            widget_type: WidgetType::Button,
            content: "{\"label\": \"Submit Changes\"}".to_string(),
            id: "btn-submit-01".to_string(),
        };

        let serialized = serde_json::to_string(&element).unwrap();
        // Ensure "type" field was correctly renamed in JSON representation
        assert!(serialized.contains("\"type\":\"Button\""));
        assert!(serialized.contains("\"content\":\"{\\\"label\\\": \\\"Submit Changes\\\"}\""));
        assert!(serialized.contains("\"id\":\"btn-submit-01\""));

        let deserialized: UiElement = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, element);
    }

    #[test]
    fn test_server_response_serialization() {
        let response = ServerResponse {
            text_response: "Hello user!".to_string(),
            ui_component: Some(UiElement {
                widget_type: WidgetType::Card,
                content: "General information".to_string(),
                id: "card-info".to_string(),
            }),
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: ServerResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.text_response, "Hello user!");
        assert!(deserialized.ui_component.is_some());
        assert_eq!(
            deserialized.ui_component.unwrap().widget_type,
            WidgetType::Card
        );
    }

    #[test]
    fn test_server_response_without_ui_component() {
        let response = ServerResponse {
            text_response: "Simple text-only answer".to_string(),
            ui_component: None,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: ServerResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.text_response, "Simple text-only answer");
        assert!(deserialized.ui_component.is_none());
    }
}
