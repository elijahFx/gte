use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rodio::{OutputStream, Sink};
use std::io::BufReader;

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
        Box::new(|_cc| Box::new(TextEditor::new())),
    )
}

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
}

impl TextEditor {
    fn new() -> Self {
        Self {
            text: String::new(),
            filename: None,
            unsaved_changes: false,
            show_save_dialog: false,
            error_message: None,
            music_playing: false,
            current_song: "Тема редактора".to_string(),
            audio_sink: None,
            _stream: None,
        }
    }

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
                _ => {
                    // Пробуем как текстовый файл
                    self.open_txt_file(&path);
                }
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
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Ошибка чтения DOCX файла: {}", e));
                        // Пробуем извлечь текст базовым методом как запасной вариант
                        let fallback_text = Self::extract_readable_text(&String::from_utf8_lossy(&bytes));
                        self.text = fallback_text;
                        self.filename = Some(path.clone());
                        self.unsaved_changes = false;
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Ошибка открытия DOCX файла: {}", e));
            }
        }
    }

    fn open_doc_file(&mut self, path: &PathBuf) {
        // Для .doc файлов используем простой метод извлечения текста
        match fs::read(path) {
            Ok(bytes) => {
                let text = Self::extract_readable_text(&String::from_utf8_lossy(&bytes));
                self.text = text;
                self.filename = Some(path.clone());
                self.unsaved_changes = false;
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Ошибка открытия DOC файла: {}", e));
            }
        }
    }

    fn extract_text_from_docx(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let docx = docx_rs::read_docx(bytes)?;
        
        let mut text = String::new();
        
        // Извлекаем текст из документа - обрабатываем children документа
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
                // Пропускаем таблицы для простоты
                text.push_str("\n[таблица]\n");
            }
            _ => {}
        }
    }

    fn extract_readable_text(content: &str) -> String {
        // Извлекаем читаемый текст из бинарного содержимого
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
        
        // Очищаем текст - убираем множественные пробелы
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
        self.text.clear();
        self.filename = None;
        self.unsaved_changes = false;
        self.error_message = None;
    }
}

impl Drop for TextEditor {
    fn drop(&mut self) {
        self.stop_music();
    }
}

impl eframe::App for TextEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    if ui.button("Отменить (Ctrl + Z)").clicked() {
                        ui.close_menu();
                    }
                    if ui.button("Повторить").clicked() {
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

                // Кнопка музыки в правом верхнем углу
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

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Информация о файле
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

                // Статистика текста
                let chars = self.count_characters();
                let words = self.count_words();
                let lines = self.count_lines();
                ui.label(format!("Символов: {}", chars));
                ui.label(format!("Слов: {}", words));
                ui.label(format!("Строк: {}", lines));

                ui.separator();

                // Информация о музыке
                let music_icon = if self.music_playing { "🎵" } else { "🔇" };
                ui.label(format!("{} {}", music_icon, self.current_song));

                // Ошибки
                if let Some(error) = &self.error_message {
                    ui.separator();
                    ui.colored_label(egui::Color32::RED, error);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Создаем область с вертикальной прокруткой и видимым скроллбаром
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

                    if response.changed() {
                        self.unsaved_changes = true;
                    }

                    if !response.has_focus() {
                        response.request_focus();
                    }
                });
        });

        if self.show_save_dialog {
            let mut open = true;
            egui::Window::new("Сохранение файла")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Имя файла не указано. Используйте 'Сохранить как'.");
                    if ui.button("OK").clicked() {
                        self.show_save_dialog = false;
                    }
                });

            if !open {
                self.show_save_dialog = false;
            }
        }
    }
}