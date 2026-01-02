use core::fmt;

pub(crate) struct DiagnosticCollector {
    pub data: Box<std::collections::BTreeMap<String, String>>,
    pub prefix: &'static str,
}

impl DiagnosticCollector {
    pub fn new(prefix: &'static str) -> Self {
        Self { data: Box::new(std::collections::BTreeMap::new()), prefix }
    }
    pub fn add_debug<T>(&mut self, key: &str, value: T)
    where
        T: fmt::Debug,
    {
        self.data.insert(format!("{}{}", self.prefix, key), format!("{:?}", value));
    }
    pub fn add_string(&mut self, key: &str, value: String) {
        self.data.insert(format!("{}{}", self.prefix, key), value);
    }
    pub fn add<T>(&mut self, key: &str, value: T)
    where
        T: fmt::Display,
    {
        self.data.insert(format!("{}{}", self.prefix, key), value.to_string());
    }

    pub fn into_diagnostic_data(self) -> Option<Box<std::collections::BTreeMap<String, String>>> {
        if self.data.is_empty() {
            None
        } else {
            Some(Box::new(*self.data))
        }
    }
}

impl Into<Option<Box<std::collections::BTreeMap<String, String>>>> for DiagnosticCollector {
    fn into(self) -> Option<Box<std::collections::BTreeMap<String, String>>> {
        self.into_diagnostic_data()
    }
}

impl Into<Box<std::collections::BTreeMap<String, String>>> for DiagnosticCollector {
    fn into(self) -> Box<std::collections::BTreeMap<String, String>> {
        self.data
    }
}
