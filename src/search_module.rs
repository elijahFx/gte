// search_module.rs
use eframe::egui;

#[derive(Default)]
pub struct SearchModule {
    pub search_text: String,
    pub case_sensitive: bool,
    pub show_search: bool,
    pub matches: Vec<(usize, usize)>, // (start, end) позиции совпадений
    pub current_match: usize,
    pub focus_search_field: bool,
}

impl SearchModule {
    pub fn new() -> Self {
        Self {
            search_text: String::new(),
            case_sensitive: false,
            show_search: false,
            matches: Vec::new(),
            current_match: 0,
            focus_search_field: false,
        }
    }

    pub fn toggle_search(&mut self) {
        self.show_search = !self.show_search;
        if self.show_search {
            self.focus_search_field = true;
        } else {
            self.search_text.clear();
            self.matches.clear();
            self.current_match = 0;
        }
    }

    pub fn get_matches(&self) -> &[(usize, usize)] {
        &self.matches
    }

    pub fn get_current_match_index(&self) -> usize {
        self.current_match
    }

    pub fn search_in_text(&mut self, text: &str) {
        self.matches.clear();
        self.current_match = 0;

        if self.search_text.is_empty() {
            return;
        }

        let search_pattern = if self.case_sensitive {
            self.search_text.clone()
        } else {
            self.search_text.to_lowercase()
        };

        let text_to_search = if self.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };

        let mut start = 0;
        while let Some(pos) = text_to_search[start..].find(&search_pattern) {
            let absolute_pos = start + pos;
            let end_pos = absolute_pos + search_pattern.len();
            self.matches.push((absolute_pos, end_pos));
            start = end_pos;
        }
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    pub fn previous_match(&mut self) {
        if !self.matches.is_empty() {
            if self.current_match == 0 {
                self.current_match = self.matches.len() - 1;
            } else {
                self.current_match -= 1;
            }
        }
    }

    pub fn get_current_match_position(&self) -> Option<(usize, usize)> {
        if self.current_match < self.matches.len() {
            Some(self.matches[self.current_match])
        } else {
            None
        }
    }

    pub fn show_search_panel(&mut self, ctx: &egui::Context) -> SearchPanelResult {
        if !self.show_search {
            return SearchPanelResult::None;
        }

        let mut result = SearchPanelResult::None;
        let mut show_search_temp = self.show_search;

        egui::Window::new("Поиск")
            .open(&mut show_search_temp)
            .default_width(300.0)
            .show(ctx, |ui| {
                let old_search_text = self.search_text.clone();
                let old_case_sensitive = self.case_sensitive;

                ui.horizontal(|ui| {
                    // Создаем уникальный ID для поля поиска
                    let search_field_id = ui.make_persistent_id("search_field");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_text)
                            .hint_text("Введите текст для поиска...")
                            .desired_width(200.0)
                            .id(search_field_id)
                    );

                    // Управление фокусом - запрашиваем фокус при первом открытии
                    if self.focus_search_field {
                        ui.memory_mut(|mem| mem.request_focus(search_field_id));
                        self.focus_search_field = false;
                    }

                    if response.changed() {
                        result = SearchPanelResult::SearchNeeded;
                    }

                    if ui.button("✕").clicked() {
                        result = SearchPanelResult::Close;
                    }
                });

                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.case_sensitive, "С учетом регистра").changed() {
                        result = SearchPanelResult::SearchNeeded;
                    }
                });

                ui.separator();

                // Информация о результатах поиска
                if !self.search_text.is_empty() {
                    if self.matches.is_empty() {
                        ui.colored_label(egui::Color32::YELLOW, "Совпадений не найдено");
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(format!("Найдено: {}", self.matches.len()));

                            if self.matches.len() > 1 {
                                let match_count = self.matches.len();
                                let current_match = self.current_match;

                                if ui.button("◀").clicked() {
                                    result = SearchPanelResult::PreviousMatch;
                                }
                                ui.label(format!("{} из {}", current_match + 1, match_count));
                                if ui.button("▶").clicked() {
                                    result = SearchPanelResult::NextMatch;
                                }
                            }
                        });
                    }
                }

                // Проверяем изменения после рендеринга
                if result == SearchPanelResult::None {
                    if old_search_text != self.search_text || old_case_sensitive != self.case_sensitive {
                        result = SearchPanelResult::SearchNeeded;
                    }
                }

                // Клавиши быстрого доступа
                ui.separator();
                ui.label("Быстрые клавиши:");
                ui.label("• Ctrl+F - открыть/закрыть поиск");
                ui.label("• F3 - следующее совпадение");
                ui.label("• Shift+F3 - предыдущее совпадение");
            });

        // Обновляем состояние окна
        if !show_search_temp {
            self.show_search = false;
        }

        result
    }

    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        let mut search_needed = false;

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.toggle_search();
            search_needed = true;
        }

        if self.show_search {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F3)) {
                self.next_match();
            }

            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F3)) {
                self.previous_match();
            }
        }

        search_needed
    }
}

#[derive(Debug, PartialEq)]
pub enum SearchPanelResult {
    None,
    SearchNeeded,
    NextMatch,
    PreviousMatch,
    Close,
}