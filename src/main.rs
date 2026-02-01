#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

mod influx;
mod ui;
use influx::InfluxClient;
use ui::AppState;

fn main() -> Result<(), eframe::Error> {
    let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "vyn InfluxDB Browser",
        options,
        Box::new(|_cc| {
            Ok(Box::new(InfluxDBApp::new(runtime)) as Box<dyn eframe::App>)
        }),
    )
}

struct InfluxDBApp {
    state: Arc<Mutex<AppState>>,
    runtime: Arc<Runtime>,
}

impl InfluxDBApp {
    fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState::default())),
            runtime,
        }
    }
}

impl eframe::App for InfluxDBApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut state = self.state.lock().unwrap();

        egui::TopBottomPanel::top("connection_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.label("Host:");
                    ui.text_edit_singleline(&mut state.host);
                    ui.label("Proxy:");
                    ui.text_edit_singleline(&mut state.proxy);

                    if ui.button("Connect").clicked() {
                        let host = state.host.clone();
                        let proxy = if state.proxy.is_empty() {
                            None
                        } else {
                            Some(state.proxy.clone())
                        };

                        let state_clone = Arc::clone(&self.state);
                        let ctx_clone = ctx.clone();

                        self.runtime.spawn(async move {
                            {
                                let mut state = state_clone.lock().unwrap();
                                state.status = "Connecting...".to_string();
                                state.is_loading = true;
                            }

                            ctx_clone.request_repaint();

                            let client = InfluxClient::new(host, proxy);
                            match client.show_databases().await {
                                Ok(dbs) => {
                                    let mut state = state_clone.lock().unwrap();
                                    state.databases = dbs.clone();
                                    state.status = format!("Connected: {} databases", dbs.len());
                                    state.is_loading = false;
                                    state.client = Some(client);
                                }
                                Err(e) => {
                                    let mut state = state_clone.lock().unwrap();
                                    state.status = format!("Error: {}", e);
                                    state.is_loading = false;
                                }
                            }
                            ctx_clone.request_repaint();
                        });
                    }
                });

                ui.separator();

                ui.group(|ui| {
                    ui.label("Query:");
                    ui.text_edit_singleline(&mut state.custom_query);

                    if ui.button("Execute").clicked() && !state.custom_query.is_empty() {
                        let query = state.custom_query.clone();
                        let db = state.selected_db.clone();
                        let client = state.client.clone();

                        if let Some(client) = client {
                            let state_clone = Arc::clone(&self.state);
                            let ctx_clone = ctx.clone();

                            self.runtime.spawn(async move {
                                {
                                    let mut state = state_clone.lock().unwrap();
                                    state.status = "Executing query...".to_string();
                                    state.is_loading = true;
                                }

                                ctx_clone.request_repaint();

                                match client.query(&query, db.as_deref()).await {
                                    Ok(result) => {
                                        let mut state = state_clone.lock().unwrap();
                                        if let Some((cols, rows)) = result {
                                            state.update_data(cols, rows.clone());
                                            state.status = format!("Query returned {} rows", rows.len());
                                        } else {
                                            state.status = "No results".to_string();
                                        }
                                        state.is_loading = false;
                                    }
                                    Err(e) => {
                                        let mut state = state_clone.lock().unwrap();
                                        state.status = format!("Error: {}", e);
                                        state.is_loading = false;
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&state.status);
                if state.is_loading {
                    ui.spinner();
                }
            });
        });

        egui::SidePanel::left("databases_panel")
            .default_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Databases");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for db in &state.databases.clone() {
                        let is_selected = state.selected_db.as_ref() == Some(db);

                        if ui.selectable_label(is_selected, db).clicked() {
                            state.selected_db = Some(db.clone());
                            let client = state.client.clone();
                            let db_name = db.clone();

                            if let Some(client) = client {
                                let state_clone = Arc::clone(&self.state);
                                let ctx_clone = ctx.clone();

                                self.runtime.spawn(async move {
                                    {
                                        let mut state = state_clone.lock().unwrap();
                                        state.status = "Loading measurements...".to_string();
                                        state.is_loading = true;
                                    }

                                    ctx_clone.request_repaint();

                                    match client.show_measurements(&db_name).await {
                                        Ok(measurements) => {
                                            let mut state = state_clone.lock().unwrap();
                                            state.measurements = measurements.clone();
                                            state.status = format!("{} measurements", measurements.len());
                                            state.is_loading = false;
                                        }
                                        Err(e) => {
                                            let mut state = state_clone.lock().unwrap();
                                            state.status = format!("Error: {}", e);
                                            state.is_loading = false;
                                        }
                                    }
                                    ctx_clone.request_repaint();
                                });
                            }
                        }
                    }
                });
            });

        egui::SidePanel::left("measurements_panel")
            .default_width(250.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Measurements");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for measurement in &state.measurements.clone() {
                        let is_selected = state.selected_measurement.as_ref() == Some(measurement);

                        if ui.selectable_label(is_selected, measurement).clicked() {
                            state.selected_measurement = Some(measurement.clone());
                            state.offset = 0;
                            state.current_data.clear();
                            state.current_data_strings.clear();

                            let client = state.client.clone();
                            let db = state.selected_db.clone();
                            let meas = measurement.clone();

                            if let Some(client) = client {
                                if let Some(db) = db {
                                    let state_clone = Arc::clone(&self.state);
                                    let ctx_clone = ctx.clone();

                                    self.runtime.spawn(async move {
                                        Self::load_chunk(state_clone, ctx_clone, client, db, meas, 0).await;
                                    });
                                }
                            }
                        }
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Load More").clicked() {
                    let client = state.client.clone();
                    let db = state.selected_db.clone();
                    let meas = state.selected_measurement.clone();
                    let offset = state.offset;

                    if let (Some(client), Some(db), Some(meas)) = (client, db, meas) {
                        let state_clone = Arc::clone(&self.state);
                        let ctx_clone = ctx.clone();

                        self.runtime.spawn(async move {
                            Self::load_chunk(state_clone, ctx_clone, client, db, meas, offset).await;
                        });
                    }
                }

                if ui.button("Export Visible").clicked() {
                    Self::export_visible(&state);
                }

                if ui.button("Export ALL").clicked() {
                    let client = state.client.clone();
                    let db = state.selected_db.clone();
                    let meas = state.selected_measurement.clone();

                    if let (Some(client), Some(db), Some(meas)) = (client, db, meas) {
                        let state_clone = Arc::clone(&self.state);
                        let ctx_clone = ctx.clone();

                        self.runtime.spawn(async move {
                            Self::export_all(state_clone, ctx_clone, client, db, meas).await;
                        });
                    }
                }

                ui.label(format!("Rows: {}", state.current_data.len()));
            });

            ui.separator();

            // Virtualized table for performance with large datasets
            if !state.current_columns.is_empty() {
                use egui_extras::{TableBuilder, Column};

                let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
                let num_columns = state.current_columns.len();

                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .columns(Column::auto().at_least(120.0).resizable(true), num_columns)
                    .header(20.0, |mut header| {
                        for col in &state.current_columns {
                            header.col(|ui| {
                                ui.strong(col);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(text_height, state.current_data_strings.len(), |mut row| {
                            let row_index = row.index();
                            if let Some(data_row) = state.current_data_strings.get(row_index) {
                                for value_str in data_row {
                                    row.col(|ui| {
                                        ui.label(value_str);
                                    });
                                }
                            }
                        });
                    });
            }
        });
    }
}

impl InfluxDBApp {
    async fn load_chunk(
        state: Arc<Mutex<AppState>>,
        ctx: egui::Context,
        client: InfluxClient,
        db: String,
        measurement: String,
        offset: usize,
    ) {
        {
            let mut state_guard = state.lock().unwrap();
            state_guard.status = format!("Loading rows from offset {}...", offset);
            state_guard.is_loading = true;
        }

        ctx.request_repaint();

        const CHUNK_SIZE: usize = 10000;
        let query = format!(
            r#"SELECT * FROM "{}" LIMIT {} OFFSET {}"#,
            measurement, CHUNK_SIZE, offset
        );

        match client.query(&query, Some(&db)).await {
            Ok(Some((cols, rows))) => {
                let mut state = state.lock().unwrap();

                if offset == 0 {
                    state.update_data(cols, rows.clone());
                } else {
                    state.extend_data(rows.clone());
                }

                state.offset += rows.len();
                state.status = format!("Loaded {} rows", state.current_data.len());
                state.is_loading = false;
            }
            Ok(None) => {
                let mut state = state.lock().unwrap();
                state.status = "No more data".to_string();
                state.is_loading = false;
            }
            Err(e) => {
                let mut state = state.lock().unwrap();
                state.status = format!("Error: {}", e);
                state.is_loading = false;
            }
        }

        ctx.request_repaint();
    }

    fn export_visible(state: &AppState) {
        if state.current_data.is_empty() {
            return;
        }

        let filename = format!(
            "{}_{}.csv",
            state.selected_measurement.as_ref().unwrap_or(&"export".to_string()),
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&filename)
            .save_file()
        {
            let mut writer = csv::Writer::from_path(path).unwrap();
            writer.write_record(&state.current_columns).unwrap();

            for row in &state.current_data {
                let row_strings: Vec<String> = row.iter().map(|v| v.to_string()).collect();
                writer.write_record(&row_strings).unwrap();
            }

            writer.flush().unwrap();
        }
    }

    async fn export_all(
        state: Arc<Mutex<AppState>>,
        ctx: egui::Context,
        client: InfluxClient,
        db: String,
        measurement: String,
    ) {
        let filename = format!(
            "{}_full_{}.csv",
            measurement,
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );

        let path = if let Some(p) = rfd::FileDialog::new()
            .set_file_name(&filename)
            .save_file()
        {
            p
        } else {
            return;
        };

        {
            let mut state_guard = state.lock().unwrap();
            state_guard.status = "Exporting all data...".to_string();
            state_guard.is_loading = true;
        }

        ctx.request_repaint();

        const CHUNK_SIZE: usize = 50000;
        let mut offset = 0;
        let mut total = 0;
        let mut writer = csv::Writer::from_path(path).unwrap();
        let mut first = true;

        loop {
            let query = format!(
                r#"SELECT * FROM "{}" LIMIT {} OFFSET {}"#,
                measurement, CHUNK_SIZE, offset
            );

            match client.query(&query, Some(&db)).await {
                Ok(Some((cols, rows))) => {
                    if first {
                        writer.write_record(&cols).unwrap();
                        first = false;
                    }

                    if rows.is_empty() {
                        break;
                    }

                    for row in &rows {
                        let row_strings: Vec<String> = row.iter().map(|v| v.to_string()).collect();
                        writer.write_record(&row_strings).unwrap();
                    }

                    total += rows.len();
                    offset += CHUNK_SIZE;

                    {
                        let mut state = state.lock().unwrap();
                        state.status = format!("Exported {} rows...", total);
                    }

                    ctx.request_repaint();

                    if rows.len() < CHUNK_SIZE {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.status = format!("Export error: {}", e);
                    state.is_loading = false;

                    ctx.request_repaint();
                    return;
                }
            }
        }

        writer.flush().unwrap();

        let mut state = state.lock().unwrap();
        state.status = format!("Export complete: {} rows", total);
        state.is_loading = false;

        ctx.request_repaint();
    }
}