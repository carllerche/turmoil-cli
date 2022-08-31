mod parse;

mod expr;
use colored::Colorize;
use expr::Expr;

use clap::Parser;
use serde_json::Value;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Turmoil log file
    path: String,

    /// Limit the number of matches to NUM
    #[clap(short = 'm', long)]
    max_count: Option<usize>,

    #[clap(short = 'C', long)]
    count: bool,

    /// Event filter
    #[clap(short = 'F', long)]
    filter: Option<Expr>,

    /// At what point to **start** showing events
    #[clap(long)]
    start: Option<Expr>,

    /// How many events to skip
    #[clap(long)]
    skip: Option<usize>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Event {
    Recv {
        host: Dot,
        src: Dot,
        elapsed: Duration,
        message: Value,
    },
    Send {
        host: Dot,
        dst: String,
        elapsed: Duration,
        delay: Option<Duration>,
        dropped: bool,
        message: Value,
    },
    Log {
        host: Dot,
        elapsed: Duration,
        line: String,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
enum Entry {
    Event {
        host: Dot,
        elapsed: Duration,
        kind: EventKind,
    },
    Message(Value),
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum EventKind {
    Recv {
        src: Dot,
    },
    Send {
        dst: String,
        delay: Option<Duration>,
        dropped: bool,
    },
    Log {
        line: String,
    },
}

#[derive(Debug, serde::Deserialize)]
struct Dot {
    host: Value,
    version: Value,
}

fn main() {
    let cli = Cli::parse();

    // let filter = cli.filter.as_ref().map(|filter| {
    //     parse::parse_str(filter)
    // });

    if false {
        process2(&cli);
    } else {
        process(&cli);
    }
}

struct Iter<'a> {
    entries: std::iter::Peekable<Box<dyn Iterator<Item = Result<Entry, serde_json::Error>> + 'a>>,
}

const ERR: &str = "unexpected log format";

impl<'a> Iter<'a> {
    fn next_message(&mut self) -> serde_json::Value {
        match self.entries.next().expect(ERR).unwrap() {
            Entry::Message(value) => value,
            _ => panic!("{}", ERR),
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        loop {
            return Some(match self.entries.next()?.unwrap() {
                Entry::Event {
                    host,
                    elapsed,
                    kind,
                } => match kind {
                    EventKind::Recv { src } => {
                        let message = self.next_message();
                        Event::Recv {
                            host,
                            src,
                            elapsed,
                            message,
                        }
                    }
                    EventKind::Send {
                        dst,
                        delay,
                        dropped,
                    } => {
                        let message = self.next_message();
                        Event::Send {
                            host,
                            dst,
                            elapsed,
                            delay,
                            dropped,
                            message,
                        }
                    }
                    EventKind::Log { line, .. } => Event::Log {
                        host,
                        elapsed,
                        line,
                    },
                },
                _ => panic!("{}", ERR),
            });
        }
    }
}

fn process2(cli: &Cli) {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(&cli.path).unwrap();
    let mut reader = BufReader::new(file);

    let de = serde_json::Deserializer::new(serde_json::de::IoRead::new(&mut reader));

    for res in de.into_iter::<Entry>().take(10) {
        println!("{:#?}", res);
    }
}

fn process(cli: &Cli) {
    use std::fs::File;
    use std::io::BufReader;

    let max_count = cli.max_count.unwrap_or(usize::MAX);

    let file = File::open(&cli.path).unwrap();
    let mut reader = BufReader::new(file);

    let de = Box::new(
        serde_json::Deserializer::new(serde_json::de::IoRead::new(&mut reader))
            .into_iter::<Entry>(),
    );
    let de = Iter {
        entries: Iterator::peekable(de),
    };

    let events = de
        // Include an event identifier
        .enumerate()
        .skip(cli.skip.unwrap_or(0))
        .skip_while(|(_, event)| {
            !cli.start
                .as_ref()
                .map(|expr| expr.matches(event))
                .unwrap_or(true)
        })
        // If a filter is specified, filter out events that don't match
        .filter(|(_, event)| matches(&event, &cli.filter))
        // If a limit is applied, only take that number
        .take(max_count);

    if cli.count {
        println!("{}", events.count());
    } else {
        for (i, event) in events {
            write(i, &event);
        }
    }
}

fn matches(event: &Event, filter: &Option<Expr>) -> bool {
    match filter {
        Some(filter) => filter.matches(event),
        None => true,
    }
}

fn write(i: usize, event: &Event) {
    match event {
        Event::Recv {
            host,
            src,
            elapsed,
            message,
        } => {
            print_head(i, event);
            println!("     Host:   {}", host);
            println!("     Origin: {}", src);
            println!("     When:   {:?}", elapsed);
            write_msg(message);
        }
        Event::Send {
            host,
            dst,
            elapsed,
            delay,
            dropped,
            message,
        } => {
            print_head(i, event);
            println!("     Host: {}", host);
            println!("     To:   {}", dst);
            println!("     When: {:?}", elapsed);

            if *dropped {
                println!("     LOST!");
            } else if let Some(delay) = delay {
                println!("     Delayed: {:?}", delay);
            }
            write_msg(message);
        }
        Event::Log {
            host,
            elapsed,
            line,
        } => {
            print_head(i, event);
            println!("     Host: {}", host);
            println!("     When: {:?}", elapsed);
            println!("");
            println!("         {}", line);
            println!("");
        }
    }
}

fn print_head(i: usize, event: &Event) {
    println!(
        "{}: {}",
        // 118ab2
        i,
        match event {
            Event::Recv { .. } => "🡰 Receive",
            Event::Send { .. } => "🡲 Send",
            Event::Log { .. } => "* Log",
            // ffd166
        }
        .truecolor(0xff, 0xd1, 0x66)
        .bold()
    );

    // 06d6a0
}

fn write_msg(msg: &serde_json::Value) {
    let writer = indent_write::io::IndentWriter::new("     ", std::io::stdout());
    println!("");
    serde_json::to_writer_pretty(writer, msg).unwrap();
    println!("");
    println!("");
}

impl fmt::Display for Dot {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            "{} @ {}",
            self.host.as_str().expect("unexpected value"),
            self.version
        )
    }
}
