use serde::{de, Deserialize, Deserializer, Serialize};
use std::sync::Arc;

/// Enum representing the ranges of pages to print
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PrintPageRange {
    /// Single page
    Integer(u64),
    /// Range of pages hyphen-separated.
    Range(Arc<str>),
}

/// Parameters of printing operation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PrintParameters {
    /// Print orientation
    pub orientation: PrintOrientation,
    /// Print scale
    #[serde(deserialize_with = "deserialize_to_print_scale_f64")]
    pub scale: f64,
    /// Print background
    pub background: bool,
    /// Dimentions of page
    pub page: PrintPage,
    /// Margins of the print
    pub margin: PrintMargins,
    /// Ranges of pages to print
    pub page_ranges: Arc<[PrintPageRange]>,
    /// Shrink page to fit
    pub shrink_to_fit: bool,
}

impl Default for PrintParameters {
    fn default() -> Self {
        PrintParameters {
            orientation: PrintOrientation::default(),
            scale: 1.0,
            background: false,
            page: PrintPage::default(),
            margin: PrintMargins::default(),
            page_ranges: Arc::new([]),
            shrink_to_fit: true,
        }
    }
}

/// Enum representing the printing orientation
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrintOrientation {
    /// Print in landscape mode
    Landscape,
    /// Print in portrait mode
    #[default]
    Portrait,
}

/// Page dimentions with units in cm
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintPage {
    /// Page width, units in cm
    #[serde(deserialize_with = "deserialize_to_positive_f64")]
    pub width: f64,
    /// Page height, units in cm
    #[serde(deserialize_with = "deserialize_to_positive_f64")]
    pub height: f64,
}

impl Default for PrintPage {
    fn default() -> Self {
        PrintPage {
            width: 21.59,
            height: 27.94,
        }
    }
}

/// Page margins
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintMargins {
    /// Top margin, units in cm
    pub top: f64,
    /// Bottom margin, units in cm
    pub bottom: f64,
    /// Left margin, units in cm
    pub left: f64,
    /// Right margin, units in cm
    pub right: f64,
}

impl Default for PrintMargins {
    fn default() -> Self {
        PrintMargins {
            top: 1.0,
            bottom: 1.0,
            left: 1.0,
            right: 1.0,
        }
    }
}

fn deserialize_to_positive_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let val = f64::deserialize(deserializer)?;
    if val < 0.0 {
        return Err(de::Error::custom(format!("{val} is negative")));
    }
    Ok(val)
}

fn deserialize_to_print_scale_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let val = f64::deserialize(deserializer)?;
    if !(0.1..=2.0).contains(&val) {
        return Err(de::Error::custom(format!("{val} is outside range 0.1-2")));
    }
    Ok(val)
}
