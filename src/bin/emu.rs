use std::{io, time::Duration};

use crossterm::{
    cursor::position,
    event::{Event, EventStream, KeyCode, MouseEvent},
};
use futures::{FutureExt, StreamExt as _};
use log::{LevelFilter, error, info};
use microchip_rs::{mcus::rtl837x::DW8051_RTL837x, traits::component::MCU};
use ratatui::{
    Terminal,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout, Position},
    prelude::{Backend, CrosstermBackend},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender},
    task::yield_now,
};
use tui_logger::TuiLoggerWidget;

#[derive(Debug, Default, PartialEq)]
enum Win {
    #[default]
    Serial,
    Log,
}

struct AppState {
    // Selected Window
    selected_win: Win,
    // Serial Send
    serial_tx: Sender<u8>,
    // Serial Receive.
    serial_rx: Receiver<u8>,
}

async fn main_loop<T: Backend>(
    terminal: &mut Terminal<T>,
    app: &mut AppState,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut console = Vec::<u8>::with_capacity(4096);
    let mut reader = EventStream::new();

    let b_sel = Style::default().fg(Color::White);
    let b_default = Style::default().fg(Color::DarkGray);

    // Main loop
    loop {
        let delay = tokio::time::sleep(Duration::from_secs(1));
        let event = reader.next().fuse();
        let uart_rx = app.serial_rx.recv();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(terminal.get_frame().area());

        select! {
            _ = delay => { },
            console_rx = uart_rx => match console_rx {
                None => {
                    error!("UART channel closed!");
                    break;
                }
                Some(c) => { console.push(c); }
            },

            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        // info!("Event::{event:?}");

                        if event == Event::Key(KeyCode::Char('c').into()) {
                            info!("Cursor position: {:?}\r", position());
                        }

                        if event == Event::Key(KeyCode::Esc.into()) {
                            break;
                        } else if let Event::Key( key ) = event {
                            if app.selected_win == Win::Serial {
                                let mut send_c: Option<u8> = None;
                                if let KeyCode::Char(val) = key.code {
                                    // info!("key: {val:?}");
                                    if val.is_ascii() {
                                        send_c = Some(val as u8);
                                    }
                                }

                                if key.code == KeyCode::Enter {
                                    send_c = Some(b'\n');
                                }

                                if let Some(key) = send_c {
                                    if let Err(err) = app.serial_tx.try_send(key) {
                                        error!("Can't send Serial {err:?}");
                                    }
                                }
                            }
                        }

                        if let Event::Mouse(MouseEvent { kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), column, row , modifiers: _ }) = event {
                            let mouse_point = Position::new(column, row);


                            for (lay, wid) in layout.iter().zip([Win::Log, Win::Serial]) {
                                if lay.contains(mouse_point) {
                                    app.selected_win = wid;
                                    break;
                                }

                            }
                        }
                        // info!("Event: {event:?}");
                    }
                    Some(Err(e)) => info!("Error: {e:?}\r"),
                    None => break,
                }
            }
        };

        terminal.draw(|frame| {
            let log_widget = TuiLoggerWidget::default().block(
                Block::default()
                    .title("Log")
                    .borders(Borders::ALL)
                    .border_style(if app.selected_win == Win::Log {
                        b_sel
                    } else {
                        b_default
                    }),
            );

            frame.render_widget(log_widget, layout[0]);

            frame.render_widget(
                Paragraph::new(String::from_utf8_lossy(&console)).block(
                    Block::new()
                        .title("Serial")
                        .borders(Borders::ALL)
                        .border_style(if app.selected_win == Win::Serial {
                            b_sel
                        } else {
                            b_default
                        }),
                ),
                layout[1],
            );
        })?;
    }

    Ok(())
}

struct SimSettings {
    uart: Option<(Sender<u8>, Receiver<u8>)>,
}

async fn simulator_tasks(settings: SimSettings) {
    let mut mcu = DW8051_RTL837x::new();
    mcu.setup();

    mcu.debug = false;

    mcu.ext_uart_0 = settings.uart;

    let prg = include_bytes!("../../data/rtlinstall.bin");

    mcu.set_program(prg.to_vec());

    loop {
        mcu.next_instruction();
        yield_now().await
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn core::error::Error>> {
    // Initialize terminal
    let mut stdout = io::stdout();

    execute!(
        stdout,
        EnterAlternateScreen,
        terminal::Clear(terminal::ClearType::All),
        EnableMouseCapture,
    )?;

    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize logger
    tui_logger::init_logger(LevelFilter::Info).unwrap();

    let (tx_send, tx_recv) = mpsc::channel(32);
    let (rx_send, rx_recv) = mpsc::channel(32);

    let app_cfg = SimSettings {
        uart: Some((rx_send, tx_recv)),
    };

    let sid = tokio::spawn(simulator_tasks(app_cfg));

    let mut app = AppState {
        selected_win: Win::default(),
        serial_tx: tx_send,
        serial_rx: rx_recv,
    };

    let ret = main_loop(&mut terminal, &mut app).await;

    sid.abort();

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    terminal::disable_raw_mode()?;

    // Cleanup

    ret
}
