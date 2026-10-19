//! Attribute bag with take-and-parse helpers. Whatever is not taken stays as extras.

use std::collections::BTreeMap;

/// Attributes that identify or contact a person. Dropped everywhere, never kept as extras.
const PERSONAL: &[&str] = &[
    "SiteContact",
    "JobSitePhone",
    "JobSiteFax",
    "Email",
    "Phone",
    "Fax",
    "FullName",
    "Password",
    "LoginName",
    "WindowsUser",
    "Address1",
    "Address2",
];

pub(crate) struct Attrs(BTreeMap<String, String>);

impl Attrs {
    pub fn new(mut map: BTreeMap<String, String>) -> Self {
        for k in PERSONAL {
            map.remove(*k);
        }
        Attrs(map)
    }
    pub fn s(&mut self, k: &str) -> String {
        self.0.remove(k).unwrap_or_default()
    }
    pub fn f(&mut self, k: &str) -> Option<f64> {
        self.0.remove(k).and_then(|v| v.trim().parse().ok())
    }
    pub fn i(&mut self, k: &str) -> Option<i64> {
        self.0.remove(k).and_then(|v| v.trim().parse().ok())
    }
    pub fn b(&mut self, k: &str) -> bool {
        self.i(k).is_some_and(|v| v != 0)
    }
    pub fn rest(self) -> BTreeMap<String, String> {
        self.0
    }
}
