// main.rs
use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rodio::{OutputStream, Sink};
use std::io::BufReader;

mod search_module;
use search_module::{SearchModule, SearchPanelResult};

fn main() -> Result<(), eframe::Error> {
    let icon_data = include_bytes!("../assets/logo.png");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Текстовый редактор Глеба")
            .with_icon(
                eframe::icon_data::from_png_bytes(icon_data)
                    .expect("Failed to load icon")
            ),
        ..Default::default()
    };

    eframe::run_native(
        "Текстовый редактор Глеба",
        options,
        Box::new(|_cc| Box::<TextEditor>::default()),
    )
}

#[derive(Default)]
struct TextEditor {
    text: String,
    filename: Option<PathBuf>,
    unsaved_changes: bool,
    show_save_dialog: bool,
    error_message: Option<String>,
    music_playing: bool,
    current_song: String,
    audio_sink: Option<Arc<Mutex<Sink>>>,
    _stream: Option<OutputStream>,
    search_module: SearchModule,
}

impl TextEditor {
    // === Базовые методы подсчета ===
    fn count_words(&self) -> usize {
        self.text
            .split_whitespace()
            .filter(|word| !word.is_empty())
            .count()
    }

    fn count_characters(&self) -> usize {
        self.text.chars().count()
    }

    fn count_lines(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.lines().count()
        }
    }

    // === Музыка ===
    fn toggle_music(&mut self) {
        if self.music_playing {
            self.stop_music();
        } else {
            self.play_music();
        }
        self.music_playing = !self.music_playing;
    }

    fn play_music(&mut self) {
        let music_paths = [
            ("assets/theme.mp3", "Тема редактора"),
            ("assets/music.mp3", "Фоновая музыка"),
            ("assets/music.wav", "Фоновая музыка"),
            ("music.mp3", "Фоновая музыка"),
            ("music.wav", "Фоновая музыка"),
        ];

        for (path, song_name) in music_paths {
            if let Ok(file) = std::fs::File::open(path) {
                if let Ok((stream, stream_handle)) = OutputStream::try_default() {
                    let sink = Sink::try_new(&stream_handle).unwrap();
                    let reader = BufReader::new(file);

                    if let Ok(source) = rodio::Decoder::new(reader) {
                        sink.append(source);
                        sink.set_volume(0.5);
                        sink.play();

                        self.audio_sink = Some(Arc::new(Mutex::new(sink)));
                        self._stream = Some(stream);
                        self.current_song = song_name.to_string();
                        self.error_message = None;
                        return;
                    }
                }
            }
        }

        self.play_fallback_tone();
    }

    fn play_fallback_tone(&mut self) {
        if let Ok((stream, stream_handle)) = OutputStream::try_default() {
            let sink = Sink::try_new(&stream_handle).unwrap();

            let source = rodio::source::SineWave::new(440.0);
            sink.append(source);
            sink.set_volume(0.1);
            sink.play();

            self.audio_sink = Some(Arc::new(Mutex::new(sink)));
            self._stream = Some(stream);
            self.current_song = "Тестовый тон".to_string();
            self.error_message = Some("Музыкальный файл не найден. Воспроизводится тестовый тон.".to_string());
        }
    }

    fn stop_music(&mut self) {
        if let Some(sink) = &self.audio_sink {
            if let Ok(sink) = sink.lock() {
                sink.stop();
            }
        }
        self.audio_sink = None;
        self._stream = None;
        self.current_song = "Музыка выключена".to_string();
    }

    // === Файловые операции ===
    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Текстовые файлы", &["txt", "doc", "docx"])
            .add_filter("Документы Word", &["doc", "docx"])
            .add_filter("Текстовые файлы", &["txt"])
            .add_filter("Все файлы", &["*"])
            .pick_file() 
        {
            match path.extension().and_then(|s| s.to_str()) {
                Some("txt") => self.open_txt_file(&path),
                Some("docx") => self.open_docx_file(&path),
                Some("doc") => self.open_doc_file(&path),
                _ => self.open_txt_file(&path),
            }
        }
    }

    fn open_txt_file(&mut self, path: &PathBuf) {
        match fs::read_to_string(path) {
            Ok(content) => {
                self.text = content;
                self.filename = Some(path.clone());
                self.unsaved_changes = false;
                self.error_message = None;
                self.search_module.matches.clear();
            }
            Err(e) => {
                self.error_message = Some(format!("Ошибка открытия TXT файла: {}", e));
            }
        }
    }

    fn open_docx_file(&mut self, path: &PathBuf) {
        match fs::read(path) {
            Ok(bytes) => {
                match Self::extract_text_from_docx(&bytes) {
                    Ok(text) => {
                        self.text = text;
                        self.filename = Some(path.clone());
                        self.unsaved_changes = false;
                        self.error_message = None;
                        self.search_module.matches.clear();
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Ошибка чтения DOCX файла: {}", e));
                        let fallback_text = Self::extract_readable_text(&String::from_utf8_lossy(&bytes));
                        self.text = fallback_text;
                        self.filename = Some(path.clone());
                        self.unsaved_changes = false;
                        self.search_module.matches.clear();
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Ошибка открытия DOCX файла: {}", e));
            }
        }
    }

    fn open_doc_file(&mut self, path: &PathBuf) {
        match fs::read(path) {
            Ok(bytes) => {
                let text = Self::extract_readable_text(&String::from_utf8_lossy(&bytes));
                self.text = text;
                self.filename = Some(path.clone());
                self.unsaved_changes = false;
                self.error_message = None;
                self.search_module.matches.clear();
            }
            Err(e) => {
                self.error_message = Some(format!("Ошибка открытия DOC файла: {}", e));
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(path) = &self.filename {
            match fs::write(path, &self.text) {
                Ok(_) => {
                    self.unsaved_changes = false;
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Ошибка сохранения файла: {}", e));
                }
            }
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Текстовые файлы", &["txt"])
            .add_filter("Все файлы", &["*"])
            .save_file() 
        {
            match fs::write(&path, &self.text) {
                Ok(_) => {
                    self.filename = Some(path);
                    self.unsaved_changes = false;
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Ошибка сохранения файла: {}", e));
                }
            }
        }
    }

    fn new_file(&mut self) {
        if self.unsaved_changes {
            self.show_save_dialog = true;
            return;
        }
        
        self.text.clear();
        self.filename = None;
        self.unsaved_changes = false;
        self.error_message = None;
        self.search_module.matches.clear();
    }

    // === Поиск ===
    fn handle_search(&mut self, ctx: &egui::Context) {
        let shortcuts_triggered_search = self.search_module.handle_shortcuts(ctx);

        let search_result = self.search_module.show_search_panel(ctx);
        
        match search_result {
            SearchPanelResult::SearchNeeded => {
                self.search_module.search_in_text(&self.text);
            }
            SearchPanelResult::NextMatch => {
                self.search_module.next_match();
            }
            SearchPanelResult::PreviousMatch => {
                self.search_module.previous_match();
            }
            SearchPanelResult::Close => {
                self.search_module.toggle_search();
            }
            SearchPanelResult::None => {}
        }

        if shortcuts_triggered_search && self.search_module.show_search {
            self.search_module.search_in_text(&self.text);
        }
    }

    // === Выделение найденных элементов ===
fn highlight_matches(&self, ui: &egui::Ui, response: &egui::Response) {
    if self.search_module.matches.is_empty() {
        return;
    }

    let painter = ui.painter();
    let rect = response.rect;
    
    // Получаем информацию о шрифте
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    
    // Разбиваем текст на строки
    let lines: Vec<&str> = self.text.lines().collect();
    
    let current_match_index = self.search_module.get_current_match_index();
    let matches = self.search_module.get_matches();
    
    for (line_index, line) in lines.iter().enumerate() {
        // Вычисляем начальную позицию этой строки в общем тексте
        let line_start = lines.iter()
            .take(line_index)
            .map(|l| l.chars().count() + 1) // +1 для символа новой строки
            .sum::<usize>();
        
        let line_end = line_start + line.chars().count();
        
        // Находим все совпадения в этой строке
        for &(start, end) in matches {
            if start >= line_start && end <= line_end {
                let is_current = matches
                    .iter()
                    .position(|&m| m == (start, end))
                    .map(|idx| idx == current_match_index)
                    .unwrap_or(false);
                
                // Вычисляем позиции для выделения
                let match_start_in_line = start - line_start;
                let match_end_in_line = end - line_start;
                
                // Приблизительный расчет позиций (моноширинный шрифт)
                let char_width = 8.0; // Ширина символа в моноширинном шрифте
                let x_start = rect.left() + (match_start_in_line as f32 * char_width);
                let x_end = rect.left() + (match_end_in_line as f32 * char_width);
                let y_top = rect.top() + (line_index as f32 * row_height);
                let y_bottom = y_top + row_height;
                
                let highlight_rect = egui::Rect::from_min_max(
                    egui::pos2(x_start, y_top),
                    egui::pos2(x_end, y_bottom)
                );
                
                // Рисуем выделение
                let color = if is_current {
                    egui::Color32::from_rgba_unmultiplied(255, 100, 100, 180) // Полупрозрачный красный
                } else {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 100, 120) // Полупрозрачный желтый
                };
                
                painter.rect_filled(highlight_rect, egui::Rounding::ZERO, color);
            }
        }
    }
}

    // === Утилиты для работы с документами ===
    fn extract_text_from_docx(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let docx = docx_rs::read_docx(bytes)?;

        let mut text = String::new();

        let document = docx.document;
        for child in &document.children {
            Self::extract_text_from_document(child, &mut text);
        }

        Ok(text.trim().to_string())
    }

    fn extract_text_from_document(document: &docx_rs::DocumentChild, text: &mut String) {
        match document {
            docx_rs::DocumentChild::Paragraph(para) => {
                for child in &para.children {
                    match child {
                        docx_rs::ParagraphChild::Run(run) => {
                            for text_child in &run.children {
                                match text_child {
                                    docx_rs::RunChild::Text(t) => {
                                        text.push_str(&t.text);
                                        text.push(' ');
                                    }
                                    docx_rs::RunChild::Break(_) => {
                                        text.push('\n');
                                    }
                                    docx_rs::RunChild::Tab(_) => {
                                        text.push('\t');
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                text.push('\n');
            }
            docx_rs::DocumentChild::Table(_) => {
                text.push_str("\n[таблица]\n");
            }
            _ => {}
        }
    }

    fn extract_readable_text(content: &str) -> String {
        let mut text = String::new();
        let mut last_char_was_text = false;

        for c in content.chars() {
            if c.is_alphabetic() || c.is_numeric() || c.is_whitespace() || c.is_ascii_punctuation() {
                text.push(c);
                last_char_was_text = true;
            } else if last_char_was_text {
                text.push(' ');
                last_char_was_text = false;
            }
        }

        let mut cleaned_text = String::new();
        let mut last_was_space = false;

        for c in text.chars() {
            if c.is_whitespace() {
                if !last_was_space {
                    cleaned_text.push(' ');
                    last_was_space = true;
                }
            } else {
                cleaned_text.push(c);
                last_was_space = false;
            }
        }

        cleaned_text.trim().to_string()
    }
}

impl Drop for TextEditor {
    fn drop(&mut self) {
        self.stop_music();
    }
}

impl eframe::App for TextEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_search(ctx);

        // Верхняя панель меню
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Файл", |ui| {
                    if ui.button("Новый").clicked() {
                        self.new_file();
                        ui.close_menu();
                    }
                    if ui.button("Открыть").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Сохранить").clicked() {
                        self.save_file();
                        ui.close_menu();
                    }
                    if ui.button("Сохранить как").clicked() {
                        self.save_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Выйти").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Редактировать", |ui| {
                    // Убрал дублирующийся пункт "Поиск"
                    if ui.button("Найти (Ctrl + F)").clicked() {
                        self.search_module.toggle_search();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Вырезать (Ctrl + X)").clicked() {
                        ui.close_menu();
                    }
                    if ui.button("Копировать (Ctrl + C)").clicked() {
                        ui.close_menu();
                    }
                    if ui.button("Вставить (Ctrl + V)").clicked() {
                        ui.close_menu();
                    }
                });

                // Кнопка музыки
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button_text = if self.music_playing { "🔊 Музыка" } else { "🔇 Музыка" };
                    let button_color = if self.music_playing { 
                        egui::Color32::from_rgb(100, 200, 100) 
                    } else { 
                        egui::Color32::from_rgb(200, 100, 100) 
                    };

                    if ui.add(
                        egui::Button::new(button_text)
                            .fill(button_color)
                            .min_size(egui::Vec2::new(100.0, 0.0))
                    ).clicked() {
                        self.toggle_music();
                    }
                });
            });
        });

        // Нижняя панель статуса
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let filename = self
                    .filename
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Без названия");

                let file_status = if self.unsaved_changes {
                    format!("{} • (изменен)", filename)
                } else {
                    filename.to_string()
                };
                ui.label(file_status);

                ui.separator();

                let chars = self.count_characters();
                let words = self.count_words();
                let lines = self.count_lines();
                ui.label(format!("Символов: {}", chars));
                ui.label(format!("Слов: {}", words));
                ui.label(format!("Строк: {}", lines));

                if !self.search_module.matches.is_empty() {
                    ui.separator();
                    ui.label(format!("Найдено: {}", self.search_module.matches.len()));
                }

                ui.separator();

                let music_icon = if self.music_playing { "🎵" } else { "🔇" };
                ui.label(format!("{} {}", music_icon, self.current_song));

                if let Some(error) = &self.error_message {
                    ui.separator();
                    ui.colored_label(egui::Color32::RED, error);
                }
            });
        });

        // Основная область текста
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(false)
                .show(ui, |ui| {
                    let text_edit = egui::TextEdit::multiline(&mut self.text)
                        .code_editor()
                        .desired_rows(30)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .frame(true);

                    let response = ui.add(text_edit);

                    // Добавляем визуальное выделение найденных совпадений
                    if !self.search_module.matches.is_empty() {
                        self.highlight_matches(ui, &response);
                    }

                    if response.changed() {
                        self.unsaved_changes = true;
                        if self.search_module.show_search && !self.search_module.search_text.is_empty() {
                            self.search_module.search_in_text(&self.text);
                        }
                    }

                    if !response.has_focus() && !self.search_module.show_search {
                        response.request_focus();
                    }
                });
        });

        // Диалог сохранения
        if self.show_save_dialog {
            let mut open = true;
            egui::Window::new("Сохранение файла")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Сохранить изменения перед созданием нового файла?");
                    ui.horizontal(|ui| {
                        if ui.button("Сохранить").clicked() {
                            self.save_file();
                            self.new_file();
                            self.show_save_dialog = false;
                        }
                        if ui.button("Не сохранять").clicked() {
                            self.new_file();
                            self.show_save_dialog = false;
                        }
                        if ui.button("Отмена").clicked() {
                            self.show_save_dialog = false;
                        }
                    });
                });

            if !open {
                self.show_save_dialog = false;
            }
        }
    }
}