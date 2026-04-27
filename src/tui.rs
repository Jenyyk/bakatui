use crate::timetable::Hour;
use crate::{Timetable, timetable::DayType};

use std::sync::mpsc::{Receiver, Sender};

use crossterm::{
    cursor::{self, MoveToNextLine},
    event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read},
    execute, queue,
    style::{Print, Stylize},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use std::io::Write;

struct Meta {
    term_height: u16,
    term_width: u16,

    virtual_cursor: (usize, usize),

    longest_abbrev: usize,

    stored_tb: Timetable,

    selected_hour: Option<Hour>,

    extra_info: bool,

    week_modifier: i64,

    backend_sender: Sender<BackendCommand>,
}

pub enum BackendCommand {
    // week modifier to send
    RefreshTimetable(i64),
    Quit,
}
use crate::FrontendCommand;

pub async fn tui_loop(
    timetable_receiver: Receiver<FrontendCommand>,
    backend_sender: Sender<BackendCommand>,
) -> Result<(), std::io::Error> {
    let mut stdout = std::io::stdout();
    let init_tb = timetable_receiver
        .recv()
        .expect("Initial timetable receive failed");
    let mut meta = Meta {
        stored_tb: match init_tb {
            FrontendCommand::TimetableData(tb) => tb,
            _ => panic!("First frontend command was not timetable data"),
        },
        backend_sender,
        ..Default::default()
    };
    update_term_size(&mut meta)?;

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    bottom_debug(&mut stdout, &meta, "Init completed")?;
    stdout.flush()?;

    loop {
        update_term_size(&mut meta)?;
        execute!(stdout, cursor::MoveTo(0, 0))?;

        // rendering here
        queue!(
            stdout,
            Clear(ClearType::CurrentLine),
            Print(format!(
                "Týden: {}",
                match meta.week_modifier {
                    ..-1 => meta.week_modifier.to_string(),
                    -1 => "minulý".into(),
                    0 => "tento".into(),
                    1 => "příští".into(),
                    _ => format!("+{}", meta.week_modifier),
                }
            )),
            cursor::MoveToNextLine(1)
        )?;
        render_timetable(&mut meta)?;
        render_selected_info(&meta)?;
        stdout.flush()?;

        // input management here
        if poll(std::time::Duration::from_millis(500))?
            && let Event::Key(key) = read().unwrap()
        {
            bottom_debug(&mut stdout, &meta, format!("{:?}", key))?;
            if handle_key(key, &mut meta).is_err() {
                // this indicates a quit action
                return Ok(());
            };
        }

        // check for new commands
        while let Ok(cmd) = timetable_receiver.try_recv() {
            match cmd {
                FrontendCommand::TimetableData(tb) => meta.stored_tb = tb,
                FrontendCommand::Log(msg) => bottom_log(&mut stdout, &meta, msg)?,
                FrontendCommand::Quit => gracefully_quit(&meta),
            };
        }
    }
}

fn update_term_size(meta: &mut Meta) -> Result<(), std::io::Error> {
    let term_size = crossterm::terminal::size()?;
    meta.term_height = term_size.1;
    meta.term_width = term_size.0;
    Ok(())
}

fn get_longest_abbrev(timetable: &Timetable) -> usize {
    timetable.days.iter().fold(0, |max, day| {
        day.hours.iter().fold(max, |max, (_, hour)| {
            std::cmp::max(max, hour.subject_short.chars().count())
        })
    })
}

fn pad_string(string: impl Into<String>, width: usize) -> String {
    let string = string.into();
    let len = string.chars().count();
    if len >= width {
        return string;
    }
    let padding = width - len;
    format!("{}{}", string, " ".repeat(padding))
}

fn bottom_log(
    stdout: &mut std::io::Stdout,
    meta: &Meta,
    message: impl Into<String>,
) -> Result<(), std::io::Error> {
    let old_pos = cursor::position()?;
    queue!(
        stdout,
        cursor::MoveTo(1, meta.term_height - 1),
        Clear(ClearType::CurrentLine),
        Print(message.into()),
        cursor::MoveTo(old_pos.0, old_pos.1)
    )?;

    Ok(())
}

fn bottom_debug(
    stdout: &mut std::io::Stdout,
    meta: &Meta,
    message: impl Into<String>,
) -> Result<(), std::io::Error> {
    if std::env::var("DEBUG").is_err() {
        return Ok(());
    }
    bottom_log(stdout, meta, message)
}

fn handle_key(key: KeyEvent, meta: &mut Meta) -> Result<(), ()> {
    match key.code {
        // quitting
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            gracefully_quit(meta);
            return Err(());
        }
        KeyCode::Char('q') => {
            gracefully_quit(meta);
            return Err(());
        }

        // moving cursor
        KeyCode::Up => meta.virtual_cursor_up(),
        KeyCode::Down => meta.virtual_cursor_down(),
        KeyCode::Left => meta.virtual_cursor_left(),
        KeyCode::Right => meta.virtual_cursor_right(),

        // hour info
        KeyCode::Enter => meta.extra_info = !meta.extra_info,

        // week manipulation
        KeyCode::Char('+') => {
            meta.week_modifier += 1;
            refresh_timetable(meta);
        }
        KeyCode::Char('-') => {
            meta.week_modifier -= 1;
            refresh_timetable(meta);
        }

        KeyCode::Char('r') => refresh_timetable(meta),

        _ => {}
    };

    Ok(())
}

fn gracefully_quit(meta: &Meta) {
    let mut stdout = std::io::stdout();

    let _ = disable_raw_mode();
    let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);

    meta.backend_sender.send(BackendCommand::Quit).unwrap();
}

impl Meta {
    fn virtual_cursor_up(&mut self) {
        self.virtual_cursor.1 = self.virtual_cursor.1.saturating_sub(1);
    }

    fn virtual_cursor_down(&mut self) {
        let max_row = self.stored_tb.days.len() - 1;
        self.virtual_cursor.1 = self.virtual_cursor.1.saturating_add(1).min(max_row);
    }

    fn virtual_cursor_left(&mut self) {
        self.virtual_cursor.0 = self.virtual_cursor.0.saturating_sub(1);
    }

    fn virtual_cursor_right(&mut self) {
        let max_col = self.stored_tb.hour_captions.len() - 1;
        self.virtual_cursor.0 = self.virtual_cursor.0.saturating_add(1).min(max_col);
    }

    fn refresh_longest_abbrev(&mut self) {
        self.longest_abbrev = get_longest_abbrev(&self.stored_tb);
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            term_height: 0,
            term_width: 0,
            virtual_cursor: (0, 0),
            longest_abbrev: 0,
            selected_hour: None,
            extra_info: false,
            week_modifier: 0,
            stored_tb: Timetable::default(),
            backend_sender: std::sync::mpsc::channel().0,
        }
    }
}

fn render_timetable(meta: &mut Meta) -> Result<(), std::io::Error> {
    let mut coords = (0, 0);
    meta.refresh_longest_abbrev();
    let tb = &meta.stored_tb;
    let mut stdout = std::io::stdout();

    for day in &tb.days {
        coords.0 = 0;
        queue!(stdout, Clear(ClearType::CurrentLine))?;
        for caption in &tb.hour_captions {
            let mut to_print = match day.hours.get(caption) {
                Some(hour) => {
                    let mut to_print =
                        pad_string(&hour.subject_short, meta.longest_abbrev + 1).stylize();
                    if let Some(change) = &hour.change {
                        to_print = to_print.on((&change.change_type).into());
                    }
                    if coords == meta.virtual_cursor {
                        meta.selected_hour = Some(hour.to_owned());
                    }
                    to_print
                }
                None => {
                    if coords == meta.virtual_cursor {
                        meta.selected_hour = None;
                    };
                    " ".repeat(meta.longest_abbrev + 1).stylize()
                }
            };

            if coords == meta.virtual_cursor {
                to_print = to_print.negative();
            }

            match day.day_type {
                DayType::WorkDay => {}
                _ => to_print = to_print.on((&day.day_type).into()),
            };

            queue!(stdout, Print(to_print))?;

            coords.0 += 1;
        }
        queue!(stdout, cursor::MoveToNextLine(1))?;
        coords.1 += 1;
    }

    Ok(())
}

fn render_selected_info(meta: &Meta) -> Result<(), std::io::Error> {
    let info_corner_start: u16 = cursor::position()?.1 + 1;
    let mut stdout = std::io::stdout();
    queue!(
        stdout,
        cursor::SavePosition,
        cursor::MoveTo(0, info_corner_start)
    )?;
    queue_clear_down_except_last()?;
    if !meta.extra_info {
        return Ok(());
    }

    let mut info_width = 0;
    let to_print = meta
        .selected_hour
        .as_ref()
        .map(|h| format!("{}", &h))
        .unwrap_or("Prázdná hodina".into());
    let lines = to_print.lines();
    let mut line_count = 0;
    queue!(stdout, cursor::MoveToNextLine(1))?;

    // main content print
    for line in lines {
        line_count += 1;
        let line_to_print = format!("| {} ", line);
        info_width = (line_to_print.chars().count()).max(info_width);
        queue!(stdout, Print(line_to_print), cursor::MoveToNextLine(1))?;
    }

    // top and bottom borders
    queue!(
        stdout,
        cursor::MoveTo(0, info_corner_start),
        Print(String::from("|") + &"‾".repeat(info_width)),
        cursor::MoveTo(0, info_corner_start + line_count as u16 + 1),
        Print(String::from("|") + &"_".repeat(info_width)),
        cursor::MoveTo(info_width as u16, info_corner_start)
    )?;

    // right border
    for _ in 0..line_count + 2 {
        queue!(stdout, Print("|"), cursor::MoveDown(1), cursor::MoveLeft(1))?;
    }

    queue!(stdout, cursor::RestorePosition)?;

    Ok(())
}

fn queue_clear_down_except_last() -> Result<(), std::io::Error> {
    let mut stdout = std::io::stdout();
    let position = cursor::position()?;
    let max_row = crossterm::terminal::size()?.1;
    for _ in position.1..max_row - 1 {
        queue!(stdout, Clear(ClearType::CurrentLine), MoveToNextLine(1))?;
    }
    queue!(stdout, cursor::MoveTo(position.0, position.1))?;
    Ok(())
}

fn refresh_timetable(meta: &mut Meta) {
    let _ = meta.backend_sender
        .send(BackendCommand::RefreshTimetable(meta.week_modifier));
}
