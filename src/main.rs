extern crate argparse;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate curl;
extern crate ical;
extern crate chrono;

// set RUST_LOG=metalab_redalert=info

use argparse::{ArgumentParser, Store, StoreOption, Print};
use curl::easy::Easy;
use std::io::BufReader;
use std::fs::File;
use std::io::prelude::*;
use chrono::{NaiveDate, NaiveTime, NaiveDateTime, Local, Timelike, Duration};

const DEFAULT_URL: &'static str = "https://metalab.at/calendar/export/ical/";

fn main() -> std::io::Result<()> {
    env_logger::init();
    let mut filename: Option<String> = None;
    let mut day: Option<String> = None;
    let mut url: String = DEFAULT_URL.to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Calculate metalab red alert time for a specific day.");
        ap.refer(&mut day).add_option(&["-d", "--day"], StoreOption, "Specify day to calculate (default is today).").metavar("YYYY-MM-DD");
        ap.refer(&mut filename).add_option(&["-o"], StoreOption, "Specify output file (uses stdout otherwise).").metavar("FILENAME");
        ap.refer(&mut url).add_option(&["--url"], Store, "Specify URL for iCal file. Defaults to the metalab server's.");
        ap.add_option(&["-V", "--version"], Print(env!("CARGO_PKG_VERSION").to_string()), "Show version");
        ap.parse_args_or_exit();
    }

    let date = {
        if let Some(date) = day {
            NaiveDate::parse_from_str(&date, "%F").unwrap()
        } else {
            Local::today().naive_local()
        }
    };
    let datestr = date.format("%Y%m%dT").to_string();

    info!("Fetching calendar…");
    let mut ics = Vec::new();
    let mut easy = Easy::new();
    easy.url(&url).expect("Invalid URL.");
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            ics.extend_from_slice(data);
            Ok(data.len())
        }).unwrap();
        transfer.perform().unwrap();
    }

    let mut red_alert = NaiveDateTime::new(date, NaiveTime::from_hms(20, 0, 0));

    let reader = ical::IcalParser::new(BufReader::new(&*ics));
    match reader.last() {
        Some(Ok(cal)) => {
            for event in cal.events {
                let mut relevant = false;
                let mut startstr = None;
                let mut endstr = None;
                for property in event.properties {
                    match property.name.as_ref() {
                        "DTSTART" => {
                            if let Some(datetime) = property.value {
                                if datetime.starts_with(&datestr) {
                                    relevant = true;
                                }
                                startstr = Some(datetime);
                            }
                        },
                        "DTEND" => {
                            if let Some(datetime) = property.value {
                                endstr = Some(datetime);
                            }
                        },
                        _ => {}
                    }
                }
                if relevant {
                    if let (Some(startstr), Some(endstr)) = (startstr, endstr) {
                        if let (Ok(start), Ok(end)) = (NaiveDateTime::parse_from_str(&startstr, "%Y%m%dT%H%M%S"), NaiveDateTime::parse_from_str(&endstr, "%Y%m%dT%H%M%S")) {
                            if (start.hour() > 14 && start.hour() < 20) || (start.hour() == 20 && start.minute() == 0) {
                                if end.date() != start.date() || end.hour() >= 20 {
                                    let new_red_alert = start - Duration::minutes(15);
                                    if new_red_alert < red_alert {
                                        red_alert = new_red_alert;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(err)) => {
            error!("Parse error: {}", err);
        }
        None => {
            error!("No calendar found!");
        }
    }
    if let Some(filename) = filename {
        let mut file = File::create(filename)?;
        write!(file, "{}", &red_alert.format("%FT%T"))?;
    } else {
        println!("{}", red_alert.format("%FT%T"));
    }

    info!("Done!");
    Ok(())
}
