use crate::p2p::FrameData;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use libp2p::Multiaddr;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::{
    collections::HashMap,
    io::{self, Stdout},
};

#[derive(Clone, Debug)]
pub enum FileDownloadState {
    Downloading,
    Completed(String), // path
    Failed,
}

#[derive(Clone, Debug)]
pub struct FileDownload {
    pub file_name: String,
    pub peer_id: String,
    pub state: FileDownloadState,
}

type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct Tui {
    terminal: Terminal,
    remote_frames: HashMap<String, (String, bool, bool)>,
    listen_addresses: Vec<Multiaddr>,
    pub messages: Vec<String>,
    pub downloads: Vec<FileDownload>,
    pub input: String,
    pub input_mode: bool,
}

impl Tui {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = ratatui::Terminal::new(backend)?;
        Ok(Self {
            terminal,
            remote_frames: HashMap::new(),
            listen_addresses: Vec::new(),
            messages: Vec::new(),
            downloads: Vec::new(),
            input: String::new(),
            input_mode: false,
        })
    }

    pub fn add_listen_address(&mut self, addr: Multiaddr) {
        self.listen_addresses.push(addr);
    }

    pub fn update_frame(&mut self, frame_data: FrameData) {
        self.remote_frames.insert(
            frame_data.peer_id,
            (
                frame_data.frame,
                frame_data.is_audio_muted,
                frame_data.is_video_muted,
            ),
        );
    }

    pub fn draw(
        &mut self,
        self_frame: &str,
        is_audio_muted: bool,
        is_video_muted: bool,
    ) -> io::Result<()> {
        let Tui {
            terminal,
            remote_frames,
            messages,
            downloads,
            input,
            input_mode,
            ..
        } = self;
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(f.size());

            let video_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            let audio_status = if is_audio_muted { " (Muted)" } else { "" };
            let video_status = if is_video_muted { " (Video Off)" } else { "" };
            let title = format!(
                "My View (q: quit, i: chat, m: mute audio{}, v: mute video{}, f: send file)",
                audio_status, video_status
            );

            let self_view = Paragraph::new(self_frame)
                .block(Block::default().title(title).borders(Borders::ALL));
            f.render_widget(self_view, video_chunks[0]);

            if !remote_frames.is_empty() {
                let (remote_frame_text, is_audio_muted, is_video_muted) =
                    remote_frames.values().next().unwrap().clone();
                let remote_peer_id = remote_frames.keys().next().unwrap().clone();

                let audio_status = if is_audio_muted { " (Muted)" } else { "" };
                let video_status = if is_video_muted { " (Video Off)" } else { "" };
                let title = format!(
                    "Peer: {} (Audio: {}{}, Video: {}{})",
                    remote_peer_id,
                    if is_audio_muted { "Off" } else { "On" },
                    audio_status,
                    if is_video_muted { "Off" } else { "On" },
                    video_status
                );

                let remote_view = Paragraph::new(remote_frame_text)
                    .block(Block::default().title(title).borders(Borders::ALL));
                f.render_widget(remote_view, video_chunks[1]);
            } else {
                let remote_view = Paragraph::new("Waiting for remote frame...")
                    .block(Block::default().title("Remote View").borders(Borders::ALL));
                f.render_widget(remote_view, video_chunks[1]);
            }

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(50),
                        Constraint::Percentage(40),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(chunks[1]);

            let message_items: Vec<ListItem> =
                messages.iter().map(|m| ListItem::new(m.as_str())).collect();
            let message_list = List::new(message_items)
                .block(Block::default().borders(Borders::ALL).title("Chat"));
            f.render_widget(message_list, right_chunks[0]);

            let download_items: Vec<ListItem> = downloads
                .iter()
                .map(|d| {
                    let state_str = match &d.state {
                        FileDownloadState::Downloading => "Downloading...",
                        FileDownloadState::Completed(path) => &format!("Done -> {}", path),
                        FileDownloadState::Failed => "Failed!",
                    };
                    let line = format!("{} from {}: {}", d.file_name, d.peer_id, state_str);
                    ListItem::new(line)
                })
                .collect();
            let download_list = List::new(download_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("File Downloads"),
            );
            f.render_widget(download_list, right_chunks[1]);

            let input_paragraph = Paragraph::new(input.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Input (Enter to send, Esc to exit)"),
            );
            f.render_widget(input_paragraph, right_chunks[2]);

            if *input_mode {
                f.set_cursor(
                    right_chunks[2].x + input.len() as u16 + 1,
                    right_chunks[2].y + 1,
                );
            }
        })?;
        Ok(())
    }

    pub fn draw_waiting_for_peers(&mut self, local_peer_id: &str) -> io::Result<()> {
        let Tui {
            terminal,
            listen_addresses,
            ..
        } = self;
        let listen_addresses_items: Vec<ListItem> = listen_addresses
            .iter()
            .map(|addr| ListItem::new(Span::raw(addr.to_string())))
            .collect();

        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(30),
                        Constraint::Percentage(40),
                        Constraint::Percentage(30),
                    ]
                    .as_ref(),
                )
                .split(size);

            let title = Paragraph::new(Text::styled(
                "Rust Meet",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center);
            f.render_widget(title, chunks[0]);

            let inner_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(5)].as_ref())
                .margin(1)
                .split(chunks[1]);

            let block = Block::default()
                .title("Waiting for Peers")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            f.render_widget(block.clone(), chunks[1]);

            let peer_id_text = Text::from(vec![Line::from(vec![
                Span::styled("Your Peer ID: ", Style::default().fg(Color::White)),
                Span::styled(
                    local_peer_id,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ])]);
            let peer_id_paragraph = Paragraph::new(peer_id_text).alignment(Alignment::Center);
            f.render_widget(peer_id_paragraph, inner_chunks[0]);

            let listen_list = List::new(listen_addresses_items)
                .block(
                    Block::default()
                        .title("Listening on")
                        .borders(Borders::NONE),
                )
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
                .highlight_symbol(">> ");
            f.render_widget(listen_list, inner_chunks[1]);

            let footer = Paragraph::new(Text::styled(
                "Users can join using your Peer ID.",
                Style::default().fg(Color::Gray),
            ))
            .alignment(Alignment::Center);
            f.render_widget(footer, chunks[2]);
        })?;
        Ok(())
    }

    pub fn draw_joining(&mut self) -> io::Result<()> {
        let Tui { terminal, .. } = self;
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(45),
                        Constraint::Percentage(10),
                        Constraint::Percentage(45),
                    ]
                    .as_ref(),
                )
                .split(size);

            let text = Text::styled(
                "Joining room...",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            );
            let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
            f.render_widget(paragraph, chunks[1]);
        })?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .unwrap();
        self.terminal.show_cursor().unwrap();
    }
}
