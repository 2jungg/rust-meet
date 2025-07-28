use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::collections::HashMap;
use std::io::{self, Stdout};
use std::time::Duration;
use crate::p2p::{FrameData, AppStatus};
use crate::video;

pub struct Tui {
    stdout: Stdout,
    should_quit: bool,
    remote_frames: HashMap<String, String>,
}

impl Tui {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        Ok(Self {
            stdout,
            should_quit: false,
            remote_frames: HashMap::new(),
        })
    }

    pub fn handle_events(&mut self, _app_status: &mut AppStatus) -> io::Result<()> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
        }
        Ok(())
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn update_frame(&mut self, frame_data: FrameData) {
        self.remote_frames
            .insert(frame_data.peer_id, frame_data.frame);
    }

    pub fn draw(&mut self, self_frame: &str) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        // Draw self frame
        execute!(self.stdout, cursor::MoveTo(0, 0))?;
        println!("My View (Press 'q' to quit):\r");
        println!("{}\r", self_frame);

        // Draw remote frames
        let mut y_offset = 2 + video::OUTPUT_HEIGHT as u16;
        for (peer_id, frame) in &self.remote_frames {
            execute!(self.stdout, cursor::MoveTo(0, y_offset))?;
            println!("Peer: {}\r", peer_id);
            println!("{}\r", frame);
            y_offset += 2 + video::OUTPUT_HEIGHT as u16;
        }

        Ok(())
    }

    pub fn draw_waiting_for_peers(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::MoveTo(0, 1))?;
        println!("Waiting for peers to join...\r");
        Ok(())
    }

    pub fn draw_joining(&mut self) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        println!("Joining room...\r");
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        execute!(self.stdout, LeaveAlternateScreen, cursor::Show).unwrap();
        terminal::disable_raw_mode().unwrap();
    }
}
