use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, Gauge, List, ListItem, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, Tabs, Wrap,
    },
    Frame, Terminal,
};
use std::io;
use std::time::Duration;
use anyhow::Result;

/// TUI 应用状态
pub struct TuiApp {
    /// 当前选中的标签页
    pub current_tab: usize,
    /// 终端输入
    pub input: String,
    /// 终端输出历史
    pub terminal_history: Vec<(String, String)>, // (类型, 内容)
    /// 日志列表
    pub logs: Vec<LogEntry>,
    /// 当前模式
    pub mode: String,
    /// 滚动状态
    pub scroll: usize,
    /// 显示新建终端对话框
    pub show_new_tab: bool,
    /// 新终端名称
    pub new_tab_name: String,
}

#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub command: String,
    pub status: LogStatus,
}

#[derive(Clone)]
pub enum LogStatus {
    Success,
    Error,
    Blocked,
}

impl TuiApp {
    pub fn new() -> Self {
        let mut terminal_history = Vec::new();
        terminal_history.push(("info".to_string(), "🦞 洛璃终端助手 v0.1.0".to_string()));
        terminal_history.push(("info".to_string(), "当前模式: 普通模式".to_string()));
        terminal_history.push(("info".to_string(), "输入 'help' 查看帮助".to_string()));
        terminal_history.push(("info".to_string(), "".to_string()));

        let logs = vec![
            LogEntry {
                timestamp: "10:30:45".to_string(),
                command: "ls -la".to_string(),
                status: LogStatus::Success,
            },
            LogEntry {
                timestamp: "10:28:12".to_string(),
                command: "rm -rf /".to_string(),
                status: LogStatus::Error,
            },
            LogEntry {
                timestamp: "10:25:33".to_string(),
                command: "curl http://malicious.com".to_string(),
                status: LogStatus::Blocked,
            },
        ];

        Self {
            current_tab: 0,
            input: String::new(),
            terminal_history,
            logs,
            mode: "普通模式".to_string(),
            scroll: 0,
            show_new_tab: false,
            new_tab_name: String::new(),
        }
    }

    pub fn execute_command(&mut self) {
        let command = self.input.clone();
        if command.is_empty() {
            return;
        }

        // 添加命令到历史
        self.terminal_history.push(("command".to_string(), format!("luoli> {}", command)));

        // 模拟执行
        match command.as_str() {
            "help" => {
                self.terminal_history.push(("output".to_string(), "可用命令:".to_string()));
                self.terminal_history.push(("output".to_string(), "  help  - 显示帮助".to_string()));
                self.terminal_history.push(("output".to_string(), "  mode  - 查看当前模式".to_string()));
                self.terminal_history.push(("output".to_string(), "  clear - 清屏".to_string()));
                self.terminal_history.push(("output".to_string(), "  exit  - 退出".to_string()));
            }
            "mode" => {
                self.terminal_history.push(("output".to_string(), format!("当前模式: {}", self.mode)));
            }
            "clear" => {
                self.terminal_history.clear();
            }
            "exit" => {
                self.terminal_history.push(("output".to_string(), "再见!".to_string()));
            }
            _ => {
                self.terminal_history.push(("success".to_string(), format!("执行: {}", command)));
                self.terminal_history.push(("output".to_string(), "命令执行成功".to_string()));
            }
        }

        self.input.clear();
        self.scroll = self.terminal_history.len().saturating_sub(10);
    }

    pub fn toggle_mode(&mut self) {
        if self.mode == "普通模式" {
            self.mode = "严格模式".to_string();
            self.terminal_history.push(("warning".to_string(), "系统: 已切换到严格模式".to_string()));
        } else {
            self.mode = "普通模式".to_string();
            self.terminal_history.push(("success".to_string(), "系统: 已切换到普通模式".to_string()));
        }
    }
}

/// 运行 TUI 应用
pub async fn run_tui() -> Result<()> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用状态
    let mut app = TuiApp::new();

    // 运行主循环
    let res = run_app(&mut terminal, &mut app).await;

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut TuiApp) -> Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(250);

    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            if app.show_new_tab {
                                app.show_new_tab = false;
                            } else {
                                return Ok(());
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        KeyCode::Tab => {
                            app.current_tab = (app.current_tab + 1) % 5;
                        }
                        KeyCode::BackTab => {
                            app.current_tab = (app.current_tab + 4) % 5;
                        }
                        KeyCode::Char('1') => app.current_tab = 0,
                        KeyCode::Char('2') => app.current_tab = 1,
                        KeyCode::Char('3') => app.current_tab = 2,
                        KeyCode::Char('4') => app.current_tab = 3,
                        KeyCode::Char('5') => app.current_tab = 4,
                        KeyCode::Char('m') => app.toggle_mode(),
                        KeyCode::Char('n') => app.show_new_tab = true,
                        KeyCode::Enter => {
                            if app.show_new_tab {
                                app.show_new_tab = false;
                                app.new_tab_name.clear();
                            } else {
                                app.execute_command();
                            }
                        }
                        KeyCode::Char(c) => {
                            if app.show_new_tab {
                                app.new_tab_name.push(c);
                            } else {
                                app.input.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if app.show_new_tab {
                                app.new_tab_name.pop();
                            } else {
                                app.input.pop();
                            }
                        }
                        KeyCode::Up => {
                            if app.scroll > 0 {
                                app.scroll -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if app.scroll < app.terminal_history.len().saturating_sub(1) {
                                app.scroll += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    // 标题栏
    let title = Paragraph::new("🦞 洛璃终端助手")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // 主内容区
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);

    // 左侧：终端区域
    render_terminal(f, app, main_chunks[0]);

    // 右侧：信息面板
    render_side_panel(f, app, main_chunks[1]);

    // 底部状态栏
    let mode_color = if app.mode == "严格模式" {
        Color::Red
    } else {
        Color::Green
    };
    
    let status = Paragraph::new(format!(
        " 模式: {} | 按 'm' 切换 | 按 'q' 退出 | 按 'n' 新建终端 | Tab 切换面板 ",
        app.mode
    ))
    .style(Style::default().fg(mode_color))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);

    // 新建终端对话框
    if app.show_new_tab {
        render_new_tab_modal(f, app);
    }
}

fn render_terminal(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    // 终端输出区域
    let output_text: Vec<Line> = app
        .terminal_history
        .iter()
        .skip(app.scroll)
        .map(|(t, content)| {
            let style = match t.as_str() {
                "command" => Style::default().fg(Color::Yellow),
                "error" => Style::default().fg(Color::Red),
                "success" => Style::default().fg(Color::Green),
                "warning" => Style::default().fg(Color::Magenta),
                "info" => Style::default().fg(Color::Cyan),
                _ => Style::default().fg(Color::White),
            };
            Line::from(Span::styled(content.clone(), style))
        })
        .collect();

    let output = Paragraph::new(output_text)
        .block(Block::default().title("终端").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(output, chunks[0]);

    // 输入区域
    let input = Paragraph::new(format!("luoli> {}", app.input))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(input, chunks[1]);
}

fn render_side_panel(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // 标签页
    let titles = vec!["日志", "安全", "统计"];
    let tabs = Tabs::new(titles.iter().map(|t| Line::from(*t)).collect())
        .select(0)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    // 日志列表
    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|log| {
            let status_color = match log.status {
                LogStatus::Success => Color::Green,
                LogStatus::Error => Color::Red,
                LogStatus::Blocked => Color::Magenta,
            };
            let status_text = match log.status {
                LogStatus::Success => "✓",
                LogStatus::Error => "✗",
                LogStatus::Blocked => "⊘",
            };
            let content = format!("{} {} {}", log.timestamp, status_text, log.command);
            ListItem::new(content).style(Style::default().fg(status_color))
        })
        .collect();

    let log_list = List::new(log_items)
        .block(Block::default().title("最近日志").borders(Borders::ALL));
    f.render_widget(log_list, chunks[1]);
}

fn render_new_tab_modal(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(60, 40, f.size());
    
    let block = Block::default()
        .title("新建终端")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let name_label = Paragraph::new("终端名称:").style(Style::default().fg(Color::White));
    f.render_widget(name_label, chunks[0]);

    let name_input = Paragraph::new(app.new_tab_name.clone())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(name_input, chunks[1]);

    let hint = Paragraph::new("按 Enter 确认, 按 q 取消")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
