#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClickPosition {
    pub x: i32,
    pub y: i32,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MacroDefinition {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub hotkey: String,
    #[serde(rename = "openKey")]
    pub open_key: String,
    #[serde(rename = "openKeyDelayMs")]
    pub open_key_delay_ms: u32,
    pub click_position: Option<ClickPosition>,
    #[serde(default = "default_true")]
    pub smart_take_all: bool,
    #[serde(default)]
    pub auto_close_on_fail: bool,
    #[serde(default)]
    pub hold_sprint: bool,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClickerSettings {
    pub version: String,
    pub macros: Vec<MacroDefinition>,
    pub theme: String,
    pub accent_color: String,
    pub language: String,
    pub minimize_to_tray: bool,
    pub always_on_top: bool,
    pub autostart: bool,
    pub last_panel: String,
    pub target_window: String,
    pub target_process: String,
    pub auto_update: bool,
}

impl Default for ClickerSettings {
    fn default() -> Self {
        Self {
            version: "1.5.7".to_string(),
            macros: vec![
                MacroDefinition {
                    id: "take_all".to_string(),
                    name: "Take All".to_string(),
                    enabled: false,
                    hotkey: "".to_string(),
                    open_key: "f".to_string(),
                    open_key_delay_ms: 300,
                    click_position: None,
                    smart_take_all: true,
                    auto_close_on_fail: false,
                    hold_sprint: false,
                },
                MacroDefinition {
                    id: "auto_walk".to_string(),
                    name: "Auto Walk".to_string(),
                    enabled: false,
                    hotkey: "KeyW".to_string(),
                    open_key: "".to_string(),
                    open_key_delay_ms: 0,
                    click_position: None,
                    smart_take_all: false,
                    auto_close_on_fail: false,
                    hold_sprint: false,
                },
                MacroDefinition {
                    id: "auto_tek_legs".to_string(),
                    name: "Auto Tek Legs".to_string(),
                    enabled: false,
                    hotkey: "ctrl".to_string(),
                    open_key: "".to_string(),
                    open_key_delay_ms: 0,
                    click_position: None,
                    smart_take_all: false,
                    auto_close_on_fail: false,
                    hold_sprint: false,
                },
                MacroDefinition {
                    id: "hold_e".to_string(),
                    name: "Hold E".to_string(),
                    enabled: false,
                    hotkey: "".to_string(),
                    open_key: "".to_string(),
                    open_key_delay_ms: 0,
                    click_position: None,
                    smart_take_all: false,
                    auto_close_on_fail: false,
                    hold_sprint: false,
                },
                MacroDefinition {
                    id: "anti_afk".to_string(),
                    name: "Anti AFK".to_string(),
                    enabled: false,
                    hotkey: "".to_string(),
                    open_key: "".to_string(),
                    open_key_delay_ms: 0,
                    click_position: None,
                    smart_take_all: false,
                    auto_close_on_fail: false,
                    hold_sprint: false,
                },
            ],
            theme: "dark".to_string(),
            accent_color: "#22c55e".to_string(),
            language: "en".to_string(),
            minimize_to_tray: false,
            always_on_top: false,
            autostart: false,
            last_panel: "macros".to_string(),
            target_window: "".to_string(),
            target_process: "".to_string(),
            auto_update: true,
        }
    }
}
