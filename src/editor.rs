use std::fs;
use std::io::{self, Write};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub struct Editor {
    content: Vec<String>,
    cursor_position: CursorPosition,
    should_quit: bool,
    filename: Option<String>,
    status_message: String,
    scroll_offset: usize, // Добавляем прокрутку
    terminal_size: (u16, u16), // Размер терминала
}

#[derive(Default)]
pub struct CursorPosition {
    pub x: usize,
    pub y: usize,
}

impl Editor {
    pub fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            content: vec![String::new()],
            cursor_position: CursorPosition::default(),
            should_quit: false,
            filename: None,
            status_message: String::from("Help: Ctrl-Q = quit, Ctrl-S = save"),
            scroll_offset: 0,
            terminal_size: (width, height),
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;

        while !self.should_quit {
            self.update_terminal_size();
            self.refresh_screen()?;
            self.process_keypress()?;
        }

        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        Ok(())
    }

    fn update_terminal_size(&mut self) {
        if let Ok((width, height)) = crossterm::terminal::size() {
            self.terminal_size = (width, height);
        }
    }

    fn refresh_screen(&mut self) -> Result<(), io::Error> {
        // Обновляем прокрутку на основе позиции курсора
        self.update_scroll();

        execute!(
            io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        )?;
        
        // Показываем только видимые строки с учетом прокрутки
        let visible_lines = (self.terminal_size.1 - 1) as usize; // -1 для статусной строки
        let end_line = (self.scroll_offset + visible_lines).min(self.content.len());
        
        for line in &self.content[self.scroll_offset..end_line] {
            println!("{}\r", line);
        }

        // Перемещаем курсор с учетом прокрутки
        let cursor_y = self.cursor_position.y.saturating_sub(self.scroll_offset);
        if cursor_y < visible_lines {
            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(
                    self.cursor_position.x as u16,
                    cursor_y as u16
                )
            )?;
        }

        // Статусная строка
        let status = format!(
            "{} | Line: {}/{}, Col: {} | Scroll: {} | {}",
            self.filename.as_deref().unwrap_or("[No Name]"),
            self.cursor_position.y + 1,
            self.content.len(),
            self.cursor_position.x + 1,
            self.scroll_offset + 1,
            self.status_message
        );
        // Обрезаем статусную строку если она слишком длинная
        let status = if status.len() > self.terminal_size.0 as usize {
            format!("{}...", &status[..self.terminal_size.0 as usize - 3])
        } else {
            status
        };
        println!("\r\n{}\r", status);

        io::stdout().flush()?;
        Ok(())
    }

    fn update_scroll(&mut self) {
        let visible_lines = (self.terminal_size.1 - 1) as usize;
        
        // Прокрутка вниз если курсор ниже видимой области
        if self.cursor_position.y >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor_position.y - visible_lines + 1;
        }
        // Прокрутка вверх если курсор выше видимой области
        else if self.cursor_position.y < self.scroll_offset {
            self.scroll_offset = self.cursor_position.y;
        }
    }

    fn process_keypress(&mut self) -> Result<(), io::Error> {
        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match (code, modifiers) {
                (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                    self.should_quit = true;
                }
                (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                    self.save_file()?;
                }
                (KeyCode::PageUp, _) => {
                    self.scroll_page_up();
                }
                (KeyCode::PageDown, _) => {
                    self.scroll_page_down();
                }
                (KeyCode::Char(c), _) => {
                    self.insert_char(c);
                }
                (KeyCode::Backspace, _) => {
                    self.delete_char();
                }
                (KeyCode::Enter, _) => {
                    self.insert_newline();
                }
                (KeyCode::Left, _) => {
                    self.move_cursor_left();
                }
                (KeyCode::Right, _) => {
                    self.move_cursor_right();
                }
                (KeyCode::Up, _) => {
                    self.move_cursor_up();
                }
                (KeyCode::Down, _) => {
                    self.move_cursor_down();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn scroll_page_up(&mut self) {
        let visible_lines = (self.terminal_size.1 - 1) as usize;
        if self.scroll_offset >= visible_lines {
            self.scroll_offset -= visible_lines;
            self.cursor_position.y = self.cursor_position.y.saturating_sub(visible_lines);
            let current_line_len = self.content[self.cursor_position.y].len();
            self.cursor_position.x = self.cursor_position.x.min(current_line_len);
        }
    }

    fn scroll_page_down(&mut self) {
        let visible_lines = (self.terminal_size.1 - 1) as usize;
        self.scroll_offset += visible_lines;
        if self.scroll_offset > self.content.len().saturating_sub(visible_lines) {
            self.scroll_offset = self.content.len().saturating_sub(visible_lines);
        }
        self.cursor_position.y = (self.cursor_position.y + visible_lines).min(self.content.len() - 1);
        let current_line_len = self.content[self.cursor_position.y].len();
        self.cursor_position.x = self.cursor_position.x.min(current_line_len);
    }

    // Остальные методы остаются без изменений...
    fn insert_char(&mut self, c: char) {
        if self.cursor_position.y >= self.content.len() {
            self.content.push(String::new());
        }
        
        let current_line = &mut self.content[self.cursor_position.y];
        
        if self.cursor_position.x <= current_line.len() {
            current_line.insert(self.cursor_position.x, c);
            self.cursor_position.x += 1;
        }
    }

    fn delete_char(&mut self) {
        if self.cursor_position.x > 0 {
            let current_line = &mut self.content[self.cursor_position.y];
            current_line.remove(self.cursor_position.x - 1);
            self.cursor_position.x -= 1;
        } else if self.cursor_position.y > 0 {
            let current_line = self.content.remove(self.cursor_position.y);
            self.cursor_position.y -= 1;
            let prev_line = &mut self.content[self.cursor_position.y];
            self.cursor_position.x = prev_line.len();
            prev_line.push_str(&current_line);
        }
    }

    fn insert_newline(&mut self) {
        let current_line = &mut self.content[self.cursor_position.y];
        let new_line = current_line.split_off(self.cursor_position.x);
        self.content.insert(self.cursor_position.y + 1, new_line);
        self.cursor_position.y += 1;
        self.cursor_position.x = 0;
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_position.x > 0 {
            self.cursor_position.x -= 1;
        } else if self.cursor_position.y > 0 {
            self.cursor_position.y -= 1;
            self.cursor_position.x = self.content[self.cursor_position.y].len();
        }
    }

    fn move_cursor_right(&mut self) {
        let current_line_len = self.content[self.cursor_position.y].len();
        if self.cursor_position.x < current_line_len {
            self.cursor_position.x += 1;
        } else if self.cursor_position.y < self.content.len() - 1 {
            self.cursor_position.y += 1;
            self.cursor_position.x = 0;
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor_position.y > 0 {
            self.cursor_position.y -= 1;
            let current_line_len = self.content[self.cursor_position.y].len();
            self.cursor_position.x = self.cursor_position.x.min(current_line_len);
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor_position.y < self.content.len() - 1 {
            self.cursor_position.y += 1;
            let current_line_len = self.content[self.cursor_position.y].len();
            self.cursor_position.x = self.cursor_position.x.min(current_line_len);
        }
    }

    fn save_file(&mut self) -> Result<(), io::Error> {
        let content = self.content.join("\n");
        
        if let Some(filename) = &self.filename {
            fs::write(filename, content)?;
            self.status_message = format!("Saved to {}", filename);
        } else {
            self.filename = Some("output.txt".to_string());
            fs::write("output.txt", content)?;
            self.status_message = String::from("Saved to output.txt");
        }
        Ok(())
    }

    pub fn open_file(mut self, filename: &str) -> Result<Self, io::Error> {
        let content = fs::read_to_string(filename)?;
        self.content = content.lines().map(String::from).collect();
        if self.content.is_empty() {
            self.content.push(String::new());
        }
        self.filename = Some(filename.to_string());
        self.status_message = format!("Opened {}", filename);
        Ok(self)
    }
}