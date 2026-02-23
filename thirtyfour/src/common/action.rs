use serde::Serialize;
use serde_repr::Serialize_repr;
use std::time::Duration;

use crate::common::{keys::TypingData, types::ElementId};

/// Trait for all Actions.
pub trait Action {
    /// Get a pause action.
    fn get_pause(duration_ms: u64) -> Self;
}

/// Null Action.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NullAction {
    /// Pause action.
    Pause {
        /// Duration of the pause in milliseconds.
        duration: u64,
    },
}

impl Action for NullAction {
    fn get_pause(duration_ms: u64) -> Self {
        NullAction::Pause {
            duration: duration_ms,
        }
    }
}

/// Key Action.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum KeyAction {
    /// Pause action.
    Pause {
        /// Duration of the pause in milliseconds.
        duration: u64,
    },
    /// Key Up action.
    KeyUp {
        /// The key to press.
        value: char,
    },
    /// Key Down action.
    KeyDown {
        /// The key to release.
        value: char,
    },
}

impl Action for KeyAction {
    fn get_pause(duration_ms: u64) -> Self {
        KeyAction::Pause {
            duration: duration_ms,
        }
    }
}

/// Mouse Button.
#[derive(Debug, Clone, Serialize_repr)]
#[repr(u8)]
pub enum MouseButton {
    /// Left mouse button.
    Left = 0,
    /// Middle mouse button.
    Middle = 1,
    /// Right mouse button.
    Right = 2,
}

/// Pointer Origin.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PointerOrigin {
    /// Pointer origin is the viewport.
    Viewport,
    /// Pointer origin is the pointer itself.
    Pointer,
    /// Pointer origin is a `WebElement`.
    #[serde(rename = "element-6066-11e4-a52e-4f735466cecf")]
    WebElement(ElementId),
}

/// Pointer Action.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PointerAction {
    /// Pause action.
    Pause {
        /// Duration of the pause in milliseconds.
        duration: u64,
    },
    /// Pointer down action.
    PointerDown {
        /// The mouse button to press.
        button: MouseButton,
        /// Duration of the action in milliseconds.
        duration: u64,
    },
    /// Pointer up action.
    PointerUp {
        /// The mouse button to release.
        button: MouseButton,
        /// Duration of the action in milliseconds.
        duration: u64,
    },
    /// Pointer move action.
    PointerMove {
        /// Duration of the action in milliseconds.
        duration: u64,
        /// The pointer origin.
        origin: PointerOrigin,
        /// The x coordinate to move to.
        x: i64,
        /// The y coordinate to move to.
        y: i64,
    },
    /// Pointer cancel action.
    PointerCancel,
}

impl Action for PointerAction {
    fn get_pause(duration_ms: u64) -> Self {
        PointerAction::Pause {
            duration: duration_ms,
        }
    }
}

/// Parameters for Pointer Actions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerParameters {
    /// The type of pointer.
    pointer_type: String,
}

/// Action Source.
#[derive(Debug, Clone, Serialize)]
pub struct ActionSource<T: Action + Clone> {
    /// The ID of the action source.
    id: String,
    /// The type of action source.
    #[serde(rename(serialize = "type"))]
    action_type: String,
    /// Parameters for the action source.
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<PointerParameters>,
    /// The actions to perform.
    actions: Vec<T>,
    /// The duration of the action source in milliseconds.
    #[serde(skip_serializing)]
    duration: u64,
}

impl<T> ActionSource<T>
where
    T: Action + Clone,
{
    /// Add the specified action to this action source.
    pub fn add_action(&mut self, action: T) {
        self.actions.push(action);
    }

    /// Add a pause action so this action source.
    pub fn pause(&mut self) {
        self.actions.push(T::get_pause(0));
    }

    /// Add a pause action with the specified duration to this action source.
    pub fn pause_for(&mut self, duration_ms: u64) {
        self.actions.push(T::get_pause(duration_ms));
    }

    /// Get the ID of this action source.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl ActionSource<KeyAction> {
    /// Create a new Key action source.
    ///
    /// Duration represents the time before an action is executed.
    /// Defaults to 0ms
    #[must_use]
    pub fn new(name: &str, duration: Option<Duration>) -> Self {
        let duration = match duration {
            Some(duration) => {
                let millis = duration.as_millis();
                u64::try_from(millis).ok().unwrap_or(u64::MAX)
            }
            None => 0,
        };

        ActionSource {
            id: name.to_owned(),
            action_type: String::from("key"),
            parameters: None,
            actions: Vec::new(),
            duration,
        }
    }

    /// Add a Key Down action.
    pub fn key_down(&mut self, value: char) {
        self.add_action(KeyAction::KeyDown {
            value,
        });
    }

    /// Add a Key Up action.
    pub fn key_up(&mut self, value: char) {
        self.add_action(KeyAction::KeyUp {
            value,
        });
    }

    /// Send multiple keys as a string of Key Up and Key Down actions.
    pub fn send_keys(&mut self, text: &TypingData) {
        for c in text.as_vec() {
            self.key_down(c);
            self.key_up(c);
        }
    }
}

/// Enum representing the type of pointer action.
#[derive(Debug, Clone, Copy)]
pub enum PointerActionType {
    /// Mouse pointer.
    Mouse,
    /// Pen pointer.
    Pen,
    /// Touch pointer.
    Touch,
}

impl ActionSource<PointerAction> {
    /// Create a new Pointer action source.
    ///
    /// Duration represents the time before an action is executed.
    /// Defaults to 250ms
    #[must_use]
    pub fn new(name: &str, action_type: PointerActionType, duration: Option<Duration>) -> Self {
        let duration = match duration {
            Some(duration) => {
                let millis = duration.as_millis();
                u64::try_from(millis).ok().unwrap_or(u64::MAX)
            }
            None => 250,
        };

        ActionSource {
            id: name.to_owned(),
            action_type: String::from("pointer"),
            parameters: Some(PointerParameters {
                pointer_type: String::from(match action_type {
                    PointerActionType::Mouse => "mouse",
                    PointerActionType::Pen => "pen",
                    PointerActionType::Touch => "touch",
                }),
            }),
            actions: Vec::new(),
            duration,
        }
    }

    /// Add a move action to the specified coordinates.
    pub fn move_to(&mut self, x: i64, y: i64) {
        self.add_action(PointerAction::PointerMove {
            duration: self.duration,
            origin: PointerOrigin::Viewport,
            x,
            y,
        });
    }

    /// Add a move action by the specified coordinates.
    pub fn move_by(&mut self, x: i64, y: i64) {
        self.add_action(PointerAction::PointerMove {
            duration: self.duration,
            origin: PointerOrigin::Pointer,
            x,
            y,
        });
    }

    /// Add a move action to the specified coordinates relative to the element.
    pub fn move_to_element(&mut self, element_id: ElementId, x: i64, y: i64) {
        self.add_action(PointerAction::PointerMove {
            duration: self.duration,
            origin: PointerOrigin::WebElement(element_id),
            x,
            y,
        });
    }

    /// Add a move action to the center of the specified element.
    pub fn move_to_element_center(&mut self, element_id: ElementId) {
        self.add_action(PointerAction::PointerMove {
            duration: self.duration,
            origin: PointerOrigin::WebElement(element_id),
            x: 0,
            y: 0,
        });
    }

    /// Add a click action.
    pub fn click(&mut self) {
        self.add_action(PointerAction::PointerDown {
            button: MouseButton::Left,
            duration: 0,
        });
        self.add_action(PointerAction::PointerUp {
            button: MouseButton::Left,
            duration: 0,
        });
    }

    /// Add a right-click action.
    pub fn context_click(&mut self) {
        self.add_action(PointerAction::PointerDown {
            button: MouseButton::Right,
            duration: 0,
        });
        self.add_action(PointerAction::PointerUp {
            button: MouseButton::Right,
            duration: 0,
        });
    }

    /// Add a click-and-hold action.
    pub fn click_and_hold(&mut self) {
        self.add_action(PointerAction::PointerDown {
            button: MouseButton::Left,
            duration: 0,
        });
    }

    /// Add a click-and-hold action on the specified element.
    pub fn click_element_and_hold(&mut self, element_id: ElementId) {
        self.move_to_element_center(element_id);
        self.click_and_hold();
    }

    /// Add a release action.
    pub fn release(&mut self) {
        self.add_action(PointerAction::PointerUp {
            button: MouseButton::Left,
            duration: 0,
        });
    }

    /// Add a double-click action.
    pub fn double_click(&mut self) {
        self.click();
        self.click();
    }

    /// Add a double-click action on the specified element.
    pub fn double_click_element(&mut self, element_id: ElementId) {
        self.move_to_element_center(element_id);
        self.double_click();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn compare_null_action(action: NullAction, value: serde_json::Value) {
        let actions: Vec<NullAction> = vec![action];
        let source = ActionSource {
            action_type: String::from("none"),
            id: String::from("null"),
            parameters: None,
            actions,
            duration: 0,
        };

        let value_got = serde_json::to_value(source);
        assert!(value_got.is_ok());
        assert_eq!(
            value_got.unwrap(),
            json!({
                "id": "null",
                "type": "none",
                "actions": [ value ]
            })
        );
    }

    #[test]
    fn test_null_action() {
        compare_null_action(
            NullAction::Pause {
                duration: 0,
            },
            json!({"type": "pause", "duration": 0}),
        );

        compare_null_action(
            NullAction::Pause {
                duration: 4,
            },
            json!({"type": "pause", "duration": 4}),
        );
    }

    fn compare_key_action(action: KeyAction, value: serde_json::Value) {
        let mut source = ActionSource::<KeyAction>::new("key", None);
        source.add_action(action);

        let value_got = serde_json::to_value(source);
        assert!(value_got.is_ok());
        assert_eq!(
            value_got.unwrap(),
            json!({
                "id": "key",
                "type": "key",
                "actions": [ value ]
            })
        );
    }

    #[test]
    fn test_key_action_pause() {
        compare_key_action(
            KeyAction::Pause {
                duration: 0,
            },
            json!({"type": "pause", "duration": 0}),
        );

        compare_key_action(
            KeyAction::Pause {
                duration: 3,
            },
            json!({"type": "pause", "duration": 3}),
        );
    }

    #[test]
    fn test_key_action_updown() {
        compare_key_action(
            KeyAction::KeyDown {
                value: 'a',
            },
            json!({"type": "keyDown", "value": 'a'}),
        );

        compare_key_action(
            KeyAction::KeyDown {
                value: '\u{e004}',
            },
            json!({
            "type": "keyDown", "value": '\u{e004}'
            }),
        );

        compare_key_action(
            KeyAction::KeyUp {
                value: 'a',
            },
            json!({"type": "keyUp", "value": 'a'}),
        );

        compare_key_action(
            KeyAction::KeyUp {
                value: '\u{e004}',
            },
            json!({
            "type": "keyUp", "value": '\u{e004}'
            }),
        );
    }

    fn compare_pointer_action(action: PointerAction, value: serde_json::Value) {
        let mut source =
            ActionSource::<PointerAction>::new("mouse", PointerActionType::Mouse, None);
        source.add_action(action);

        let value_got = serde_json::to_value(source);
        assert!(value_got.is_ok());
        assert_eq!(
            value_got.unwrap(),
            json!({
                "id": "mouse",
                "type": "pointer",
                "parameters": {
                    "pointerType": "mouse"
                },
                "actions": [ value ]
            })
        );
    }

    #[test]
    fn test_pointer_action_pause() {
        compare_pointer_action(
            PointerAction::Pause {
                duration: 0,
            },
            json!({"type": "pause", "duration": 0}),
        );

        compare_pointer_action(
            PointerAction::Pause {
                duration: 2,
            },
            json!({"type": "pause", "duration": 2}),
        );
    }

    #[test]
    fn test_pointer_action_button() {
        compare_pointer_action(
            PointerAction::PointerDown {
                button: MouseButton::Left,
                duration: 0,
            },
            json!({"type": "pointerDown", "button": 0, "duration": 0}),
        );

        compare_pointer_action(
            PointerAction::PointerDown {
                button: MouseButton::Middle,
                duration: 0,
            },
            json!({"type": "pointerDown", "button": 1, "duration": 0}),
        );

        compare_pointer_action(
            PointerAction::PointerDown {
                button: MouseButton::Right,
                duration: 0,
            },
            json!({"type": "pointerDown", "button": 2, "duration": 0}),
        );

        compare_pointer_action(
            PointerAction::PointerUp {
                button: MouseButton::Left,
                duration: 0,
            },
            json!({"type": "pointerUp", "button": 0, "duration": 0}),
        );

        compare_pointer_action(
            PointerAction::PointerUp {
                button: MouseButton::Middle,
                duration: 0,
            },
            json!({"type": "pointerUp", "button": 1, "duration": 0}),
        );

        compare_pointer_action(
            PointerAction::PointerUp {
                button: MouseButton::Right,
                duration: 0,
            },
            json!({"type": "pointerUp", "button": 2, "duration": 0}),
        );
    }

    #[test]
    fn test_pointer_action_pointermove() {
        compare_pointer_action(
            PointerAction::PointerMove {
                duration: 0,
                x: 0,
                y: 0,
                origin: PointerOrigin::Viewport,
            },
            json!({
            "type": "pointerMove", "origin": "viewport", "x": 0, "y": 0, "duration": 0
            }),
        );

        compare_pointer_action(
            PointerAction::PointerMove {
                duration: 0,
                x: 0,
                y: 0,
                origin: PointerOrigin::Pointer,
            },
            json!({
            "type": "pointerMove", "origin": "pointer", "x": 0, "y": 0, "duration": 0
            }),
        );

        compare_pointer_action(
            PointerAction::PointerMove {
                duration: 0,
                x: 0,
                y: 0,
                origin: PointerOrigin::WebElement(ElementId::from("id1234")),
            },
            json!({
            "type": "pointerMove", "origin": {"element-6066-11e4-a52e-4f735466cecf": "id1234"}, "x": 0, "y": 0, "duration": 0
            }),
        );

        compare_pointer_action(
            PointerAction::PointerMove {
                duration: 1,
                x: 100,
                y: 200,
                origin: PointerOrigin::Viewport,
            },
            json!({
                "type": "pointerMove",
                "x": 100,
                "y": 200,
                "duration": 1,
                "origin": "viewport"
            }),
        );

        compare_pointer_action(
            PointerAction::PointerMove {
                duration: 1,
                x: 100,
                y: 200,
                origin: PointerOrigin::Pointer,
            },
            json!({
                "type": "pointerMove",
                "x": 100,
                "y": 200,
                "duration": 1,
                "origin": "pointer"
            }),
        );

        compare_pointer_action(
            PointerAction::PointerMove {
                duration: 1,
                x: 100,
                y: 200,
                origin: PointerOrigin::WebElement(ElementId::from("someid")),
            },
            json!({
                "type": "pointerMove",
                "x": 100,
                "y": 200,
                "duration": 1,
                "origin": {"element-6066-11e4-a52e-4f735466cecf": "someid"}
            }),
        );
    }

    #[test]
    fn test_pointer_action_cancel() {
        compare_pointer_action(PointerAction::PointerCancel, json!({"type": "pointerCancel"}));
    }
}
