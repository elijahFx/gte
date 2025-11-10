use std::fs;
use std::io::{self, Write};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, style,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    QueueableCommand,
};

pub struct Editor {
    content: Vec<String>,
    cursor_position: CursorPosition,
    should_quit: bool,
    filename: Option<String>,
    status_message: String,
    scroll_offset: usize,
    terminal_size: (u16, u16),
    search_mode: bool,          // Режим поиска
    search_query: String,       // Текст для поиска
    search_matches: Vec<Match>, // Найденные совпадения
    current_match: usize,       // Текущее выделенное совпадение
}

#[derive(Default)]
pub struct CursorPosition {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone)]
struct Match {
    line: usize,
    start: usize,
    end: usize,
}

impl Editor {
    pub fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            content: vec![String::new()],
            cursor_position: CursorPosition::default(),
            should_quit: false,
            filename: None,
            status_message: String::from("Help: Ctrl-Q = quit, Ctrl-S = save, Ctrl-F = search"),
            scroll_offset: 0,
            terminal_size: (width, height),
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: 0,
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
        self.update_scroll();

        execute!(
            io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        )?;
        
        // Показываем только видимые строки с учетом прокрутки
        let visible_lines = (self.terminal_size.1 - 2) as usize; // -2 для статусных строк
        let end_line = (self.scroll_offset + visible_lines).min(self.content.len());
        
        for (line_index, line) in self.content[self.scroll_offset..end_line].iter().enumerate() {
            let absolute_line = line_index + self.scroll_offset;
            
            if self.search_mode && !self.search_query.is_empty() {
                // В режиме поиска выделяем совпадения
                self.print_line_with_highlights(absolute_line, line)?;
            } else {
                println!("{}\r", line);
            }
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

        // Строка поиска (если активен режим поиска)
        if self.search_mode {
            let search_prompt = format!("Search: {}", self.search_query);
            let search_info = if !self.search_matches.is_empty() {
                format!(" [{} matches, current: {}]", self.search_matches.len(), self.current_match + 1)
            } else if !self.search_query.is_empty() {
                " [no matches]".to_string()
            } else {
                String::new()
            };
            
            let full_search_line = format!("{}{}", search_prompt, search_info);
            let search_line = if full_search_line.len() > self.terminal_size.0 as usize {
                format!("{}...", &full_search_line[..self.terminal_size.0 as usize - 3])
            } else {
                full_search_line
            };
            
            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(0, (self.terminal_size.1 - 2) as u16),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine),
                SetForegroundColor(Color::Yellow),
                Print(search_line),
                SetForegroundColor(Color::Reset)
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
        let status = if status.len() > self.terminal_size.0 as usize {
            format!("{}...", &status[..self.terminal_size.0 as usize - 3])
        } else {
            status
        };
        
        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(0, (self.terminal_size.1 - 1) as u16),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine),
            Print(status)
        )?;

        io::stdout().flush()?;
        Ok(())
    }

    fn print_line_with_highlights(&self, line_num: usize, line: &str) -> Result<(), io::Error> {
        let mut stdout = io::stdout();
        let mut last_pos = 0;
        
        // Получаем все совпадения для этой строки
        let line_matches: Vec<&Match> = self.search_matches
            .iter()
            .filter(|m| m.line == line_num)
            .collect();
        
        if line_matches.is_empty() {
            // Если нет совпадений, просто печатаем строку
            stdout.queue(Print(line))?;
            stdout.queue(Print("\r\n"))?;
        } else {
            // Печатаем строку с выделением совпадений
            for mat in line_matches {
                // Текст до совпадения
                if mat.start > last_pos {
                    stdout.queue(Print(&line[last_pos..mat.start]))?;
                }
                
                // Выделенное совпадение
                let is_current = self.current_match < self.search_matches.len() && 
                               self.search_matches[self.current_match].line == line_num &&
                               self.search_matches[self.current_match].start == mat.start;
                
                if is_current {
                    // Текущее совпадение выделяем другим цветом
                    stdout.queue(SetBackgroundColor(Color::Red))?;
                    stdout.queue(SetForegroundColor(Color::White))?;
                } else {
                    stdout.queue(SetBackgroundColor(Color::Yellow))?;
                    stdout.queue(SetForegroundColor(Color::Black))?;
                }
                
                stdout.queue(Print(&line[mat.start..mat.end]))?;
                stdout.queue(SetBackgroundColor(Color::Reset))?;
                stdout.queue(SetForegroundColor(Color::Reset))?;
                
                last_pos = mat.end;
            }
            
            // Текст после последнего совпадения
            if last_pos < line.len() {
                stdout.queue(Print(&line[last_pos..]))?;
            }
            
            stdout.queue(Print("\r\n"))?;
        }
        
        stdout.flush()?;
        Ok(())
    }

    fn update_scroll(&mut self) {
        let visible_lines = (self.terminal_size.1 - 2) as usize;
        
        if self.cursor_position.y >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor_position.y - visible_lines + 1;
        } else if self.cursor_position.y < self.scroll_offset {
            self.scroll_offset = self.cursor_position.y;
        }
    }

    fn process_keypress(&mut self) -> Result<(), io::Error> {
        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            if self.search_mode {
                self.process_search_keypress(code, modifiers)?;
            } else {
                self.process_normal_keypress(code, modifiers)?;
            }
        }
        Ok(())
    }

    fn process_normal_keypress(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<(), io::Error> {
        match (code, modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.save_file()?;
            }
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.enter_search_mode();
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
        Ok(())
    }

    fn process_search_keypress(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<(), io::Error> {
        match (code, modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Enter, _) => {
                self.exit_search_mode();
            }
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.find_next_match();
            }
            (KeyCode::Backspace, _) => {
                if !self.search_query.is_empty() {
                    self.search_query.pop();
                    self.perform_search();
                }
            }
            (KeyCode::Char(c), _) => {
                self.search_query.push(c);
                self.perform_search();
            }
            _ => {}
        }
        Ok(())
    }

    fn enter_search_mode(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
        self.search_matches.clear();
        self.current_match = 0;
        self.status_message = "Search mode: Type to search, Enter to exit".to_string();
    }

    fn exit_search_mode(&mut self) {
        self.search_mode = false;
        self.search_matches.clear();
        self.current_match = 0;
        self.status_message = "Help: Ctrl-Q = quit, Ctrl-S = save, Ctrl-F = search".to_string();
    }

    fn perform_search(&mut self) {
        self.search_matches.clear();
        self.current_match = 0;

        if self.search_query.is_empty() {
            return;
        }

        for (line_num, line) in self.content.iter().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(&self.search_query) {
                let absolute_pos = start + pos;
                let end_pos = absolute_pos + self.search_query.len();
                self.search_matches.push(Match {
                    line: line_num,
                    start: absolute_pos,
                    end: end_pos,
                });
                start = end_pos;
            }
        }

        if !self.search_matches.is_empty() {
            self.jump_to_match(0);
        }
    }

    fn find_next_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.search_matches.len();
            self.jump_to_match(self.current_match);
        }
    }

    fn jump_to_match(&mut self, match_index: usize) {
        if match_index < self.search_matches.len() {
            let mat = &self.search_matches[match_index];
            self.cursor_position.y = mat.line;
            self.cursor_position.x = mat.start;
            self.current_match = match_index;
        }
    }

    // Остальные методы остаются без изменений...
    fn scroll_page_up(&mut self) {
        let visible_lines = (self.terminal_size.1 - 2) as usize;
        if self.scroll_offset >= visible_lines {
            self.scroll_offset -= visible_lines;
            self.cursor_position.y = self.cursor_position.y.saturating_sub(visible_lines);
            let current_line_len = self.content[self.cursor_position.y].len();
            self.cursor_position.x = self.cursor_position.x.min(current_line_len);
        }
    }

    fn scroll_page_down(&mut self) {
        let visible_lines = (self.terminal_size.1 - 2) as usize;
        self.scroll_offset += visible_lines;
        if self.scroll_offset > self.content.len().saturating_sub(visible_lines) {
            self.scroll_offset = self.content.len().saturating_sub(visible_lines);
        }
        self.cursor_position.y = (self.cursor_position.y + visible_lines).min(self.content.len() - 1);
        let current_line_len = self.content[self.cursor_position.y].len();
        self.cursor_position.x = self.cursor_position.x.min(current_line_len);
    }

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