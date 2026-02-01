use serde_json::Value;
use crate::influx::InfluxClient;

pub struct AppState {
    pub host: String,
    pub proxy: String,
    pub databases: Vec<String>,
    pub measurements: Vec<String>,
    pub selected_db: Option<String>,
    pub selected_measurement: Option<String>,
    pub current_columns: Vec<String>,
    pub current_data: Vec<Vec<Value>>,
    pub current_data_strings: Vec<Vec<String>>, // Cached string representation
    pub custom_query: String,
    pub status: String,
    pub is_loading: bool,
    pub offset: usize,
    pub client: Option<InfluxClient>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            host: String::new(),
            proxy: String::new(),
            databases: Vec::new(),
            measurements: Vec::new(),
            selected_db: None,
            selected_measurement: None,
            current_columns: Vec::new(),
            current_data: Vec::new(),
            current_data_strings: Vec::new(),
            custom_query: String::new(),
            status: "Ready".to_string(),
            is_loading: false,
            offset: 0,
            client: None,
        }
    }
}

impl AppState {
    pub fn update_data(&mut self, columns: Vec<String>, data: Vec<Vec<Value>>) {
        self.current_columns = columns;
        // Pre-convert all values to strings for faster rendering
        self.current_data_strings = data.iter()
            .map(|row| row.iter().map(|v| value_to_string(v)).collect())
            .collect();
        self.current_data = data;
    }

    pub fn extend_data(&mut self, data: Vec<Vec<Value>>) {
        let new_strings: Vec<Vec<String>> = data.iter()
            .map(|row| row.iter().map(|v| value_to_string(v)).collect())
            .collect();
        self.current_data_strings.extend(new_strings);
        self.current_data.extend(data);
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}