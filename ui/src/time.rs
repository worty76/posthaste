//! The clock, the machine's time zone and the browser's date formatting.
//! `logic::dates` works in calendar days; this is where an instant becomes
//! a local day and a day becomes words in the reader's language.

use js_sys::{Array, Date, Intl, Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::logic::dates::{Day, WeekStart};

pub fn now_ms() -> i64 {
    Date::now() as i64
}

/// The local calendar day containing the instant `ms`.
pub fn day_of(ms: i64) -> Day {
    let date = Date::new(&JsValue::from_f64(ms as f64));
    Day {
        year: date.get_full_year() as i32,
        month: date.get_month() as u8 + 1,
        day: date.get_date() as u8,
    }
}

pub fn today() -> Day {
    day_of(now_ms())
}

fn local_midnight(day: Day) -> Date {
    // Built from calendar fields, so a daylight-saving day still starts at
    // its own midnight.
    Date::new_with_year_month_day(
        day.year as u32,
        i32::from(day.month) - 1,
        i32::from(day.day),
    )
}

/// The instant the local day starts.
pub fn start_of(day: Day) -> i64 {
    local_midnight(day).get_time() as i64
}

/// `[from, to)` of the local day. 23 or 25 hours apart on a clock-change day.
pub fn bounds(day: Day) -> (i64, i64) {
    (start_of(day), start_of(day.plus_days(1)))
}

/// The locale to format dates in for an interface language. The browser's
/// own locale when it is a variety of that language (so British and
/// American English each keep their date order), else the language itself.
pub fn locale_for(language: &str) -> String {
    let browser = leptos::prelude::window()
        .navigator()
        .language()
        .unwrap_or_default();
    let same_language = browser
        .split('-')
        .next()
        .is_some_and(|primary| primary.eq_ignore_ascii_case(language));
    if same_language {
        browser
    } else {
        language.to_owned()
    }
}

/// The first day of the week in `locale`. Monday when the browser cannot say.
pub fn week_start(locale: &str) -> WeekStart {
    match first_weekday(locale) {
        Some(7) => WeekStart::Sunday,
        Some(6) => WeekStart::Saturday,
        _ => WeekStart::Monday,
    }
}

/// `Intl.Locale` week info: 1 Monday to 7 Sunday. Browsers expose it as a
/// method or, in older ones, a property.
fn first_weekday(locale: &str) -> Option<u32> {
    let intl = Reflect::get(&js_sys::global(), &"Intl".into()).ok()?;
    let constructor: js_sys::Function = Reflect::get(&intl, &"Locale".into())
        .ok()?
        .dyn_into()
        .ok()?;
    let locale = Reflect::construct(&constructor, &Array::of1(&locale.into())).ok()?;
    let method = Reflect::get(&locale, &"getWeekInfo".into()).ok()?;
    let info = match method.dyn_ref::<js_sys::Function>() {
        Some(get) => get.call0(&locale).ok()?,
        None => Reflect::get(&locale, &"weekInfo".into()).ok()?,
    };
    Reflect::get(&info, &"firstDay".into())
        .ok()?
        .as_f64()
        .map(|n| n as u32)
}

/// How long a part of a date is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// "M"
    Narrow,
    /// "Mon", "Nov"
    Short,
    /// "Monday", "November"
    Long,
}

impl Width {
    fn as_str(self) -> &'static str {
        match self {
            Width::Narrow => "narrow",
            Width::Short => "short",
            Width::Long => "long",
        }
    }
}

/// Which parts of a date to write. The locale decides their order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateStyle {
    pub weekday: Option<Width>,
    pub month: Option<Width>,
    pub day: bool,
    pub year: bool,
}

/// Writes a day the way `locale` does.
pub fn format_day(day: Day, locale: &str, style: DateStyle) -> String {
    let options = Object::new();
    let set = |name: &str, value: &str| {
        let _ = Reflect::set(&options, &name.into(), &value.into());
    };
    if let Some(width) = style.weekday {
        set("weekday", width.as_str());
    }
    if let Some(width) = style.month {
        set("month", width.as_str());
    }
    if style.day {
        set("day", "numeric");
    }
    if style.year {
        set("year", "numeric");
    }
    let formatter = Intl::DateTimeFormat::new(&Array::of1(&locale.into()), &options);
    formatter
        .format()
        .call1(&JsValue::NULL, &local_midnight(day))
        .ok()
        .and_then(|text| text.as_string())
        .unwrap_or_else(|| day.key())
}

/// Writes the time of day of the instant `ms` the way `locale` does:
/// "3:40 PM", "15:40".
pub fn format_time(ms: i64, locale: &str) -> String {
    let options = Object::new();
    let _ = Reflect::set(&options, &"hour".into(), &"numeric".into());
    let _ = Reflect::set(&options, &"minute".into(), &"2-digit".into());
    let formatter = Intl::DateTimeFormat::new(&Array::of1(&locale.into()), &options);
    formatter
        .format()
        .call1(&JsValue::NULL, &Date::new(&JsValue::from_f64(ms as f64)))
        .ok()
        .and_then(|text| text.as_string())
        .unwrap_or_default()
}

/// The instant `ms` in the machine-readable form `<time datetime>` takes.
pub fn iso(ms: i64) -> String {
    Date::new(&JsValue::from_f64(ms as f64))
        .to_iso_string()
        .into()
}

/// The local date and time of the instant `ms`, written so it can be part
/// of a file name on every system: "2026-11-15 15.40".
pub fn file_stamp(ms: i64) -> String {
    let date = Date::new(&JsValue::from_f64(ms as f64));
    format!(
        "{}-{:02}-{:02} {:02}.{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes()
    )
}

/// How far local time is ahead of UTC right now, in minutes.
pub fn utc_offset_minutes() -> i32 {
    -(Date::new_0().get_timezone_offset() as i32)
}
