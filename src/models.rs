use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntryOut {
    pub id: String,
    pub name: Option<String>,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub mime_types: Vec<String>,
    pub actions: Vec<DesktopActionOut>,
    pub type_: Option<String>,
    pub startup_wm_class: Option<String>,
    pub startup_notify: Option<bool>,
    pub nodisplay: Option<bool>,
    pub hidden: Option<bool>,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopActionOut {
    pub id: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub scanned_roots: Vec<String>,
    pub found_count: usize,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedScanResult {
    pub scanned_roots: Vec<String>,
    pub found_count: usize,
    pub parsed_count: usize,
    pub parse_failed: usize,
    pub entries: Vec<DesktopEntryIndexed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntryIndexed {
    pub out: DesktopEntryOut,
    pub norm: String,
    pub id_lc: String,
    pub name_lc: Option<String>,
}
