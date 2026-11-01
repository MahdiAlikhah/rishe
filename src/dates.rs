//! Jalali (Solar Hijri) calendar conversion and date parsing.
//!
//! The conversion follows the Borkowski algorithm as implemented by
//! `jalaali-js`; see `NOTICE.md`.

use crate::transliterate::{persian_digit, utf8_digit};

const BREAKS: [i32; 20] = [
    -61, 9, 38, 199, 426, 686, 756, 818, 1111, 1181, 1210, 1635, 2060, 2097, 2192, 2262, 2324,
    2394, 2456, 3178,
];

const MONTHS: [&str; 12] = [
    "فروردین",
    "اردیبهشت",
    "خرداد",
    "تیر",
    "مرداد",
    "شهریور",
    "مهر",
    "آبان",
    "آذر",
    "دی",
    "بهمن",
    "اسفند",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jalali {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gregorian {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

struct JalCal {
    leap: i32,
    gy: i32,
    march: i32,
}

impl Jalali {
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if day == 0 || day > Self::month_days(year, month)? {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn is_leap(year: i32) -> bool {
        jal_cal(year).is_some_and(|cal| cal.leap == 0)
    }

    pub fn month_days(year: i32, month: u8) -> Option<u8> {
        let cal = jal_cal(year)?;
        Some(match month {
            1..=6 => 31,
            7..=11 => 30,
            12 => {
                if cal.leap == 0 {
                    30
                } else {
                    29
                }
            }
            _ => return None,
        })
    }

    pub fn to_gregorian(self) -> Option<Gregorian> {
        if self.day == 0 || self.day > Self::month_days(self.year, self.month)? {
            return None;
        }
        let day_number = j2d(self.year, i32::from(self.month), i32::from(self.day))?;
        let (year, month, day) = d2g(day_number);
        Some(Gregorian {
            year,
            month: month as u8,
            day: day as u8,
        })
    }

    pub fn from_gregorian(date: Gregorian) -> Option<Self> {
        if date.day == 0 || date.day > Gregorian::month_days(date.year, date.month)? {
            return None;
        }
        d2j(g2d(date.year, i32::from(date.month), i32::from(date.day)))
    }

    /// `۱۲ مرداد ۱۴۰۲`
    pub fn format_persian(self) -> String {
        let month = self
            .month
            .checked_sub(1)
            .and_then(|month| MONTHS.get(usize::from(month)))
            .copied()
            .unwrap_or_default();
        format!(
            "{} {} {}",
            utf8_digits(&self.day.to_string()),
            month,
            utf8_digits(&self.year.to_string())
        )
    }

    /// `۱۴۰۲/۰۵/۱۲`
    pub fn format_numeric(self) -> String {
        utf8_digits(&format!(
            "{:04}/{:02}/{:02}",
            self.year, self.month, self.day
        ))
    }

    /// Gregorian `2023-08-03`.
    pub fn to_iso(self) -> Option<String> {
        self.to_gregorian().map(Gregorian::to_iso)
    }
}

impl Gregorian {
    pub fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    pub fn month_days(year: i32, month: u8) -> Option<u8> {
        Some(match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_leap(year) {
                    29
                } else {
                    28
                }
            }
            _ => return None,
        })
    }

    pub fn to_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Parse a Jalali date from `۱۴۰۲/۰۵/۱۲`, `12-5-1402` (day first when the year
/// is last) or `۱۲ مرداد ۱۴۰۲`.
pub fn parse(text: &str) -> Option<Jalali> {
    let normalized: String = text
        .trim()
        .trim_matches(|ch: char| ch.is_whitespace() || ".,،;؛:()".contains(ch))
        .chars()
        .map(|ch| persian_digit(ch).unwrap_or(ch))
        .collect();
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return None;
    }
    parse_numeric(normalized).or_else(|| parse_named(normalized))
}

/// Format a Gregorian date in Jalali: `۱۲ مرداد ۱۴۰۲`.
pub fn format(date: Gregorian) -> Option<String> {
    Jalali::from_gregorian(date).map(Jalali::format_persian)
}

fn parse_numeric(text: &str) -> Option<Jalali> {
    let separator = text.chars().find(|ch| matches!(ch, '/' | '-' | '.'))?;
    let parts: Vec<&str> = text.split(separator).map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let (year, month, day) = if parts[0].len() == 4 {
        (parts[0], parts[1], parts[2])
    } else if parts[2].len() == 4 {
        (parts[2], parts[1], parts[0])
    } else {
        return None;
    };
    Jalali::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
}

fn parse_named(text: &str) -> Option<Jalali> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    let month = month_from_name(parts[1])?;
    Jalali::new(parts[2].parse().ok()?, month, parts[0].parse().ok()?)
}

fn month_from_name(name: &str) -> Option<u8> {
    let name = name.strip_suffix("ماه").unwrap_or(name);
    let name = if name == "امرداد" {
        "مرداد"
    } else {
        name
    };
    MONTHS
        .iter()
        .position(|month| *month == name)
        .map(|index| index as u8 + 1)
}

fn utf8_digits(text: &str) -> String {
    text.chars()
        .map(|ch| utf8_digit(ch).unwrap_or(ch))
        .collect()
}

fn div(a: i32, b: i32) -> i32 {
    a / b
}

fn modulo(a: i32, b: i32) -> i32 {
    a % b
}

fn jal_cal(year: i32) -> Option<JalCal> {
    let last = BREAKS.len() - 1;
    if year < BREAKS[0] || year >= BREAKS[last] {
        return None;
    }

    let gy = year + 621;
    let mut leap_j = -14;
    let mut jp = BREAKS[0];
    let mut jump = 0;

    for &jm in &BREAKS[1..] {
        jump = jm - jp;
        if year < jm {
            break;
        }
        leap_j += div(jump, 33) * 8 + div(modulo(jump, 33), 4);
        jp = jm;
    }

    let mut n = year - jp;
    leap_j += div(n, 33) * 8 + div(modulo(n, 33) + 3, 4);
    if modulo(jump, 33) == 4 && jump - n == 4 {
        leap_j += 1;
    }

    let leap_g = div(gy, 4) - div((div(gy, 100) + 1) * 3, 4) - 150;
    let march = 20 + leap_j - leap_g;

    if jump - n < 6 {
        n = n - jump + div(jump + 4, 33) * 33;
    }
    let mut leap = modulo(modulo(n + 1, 33) - 1, 4);
    if leap == -1 {
        leap = 4;
    }

    Some(JalCal { leap, gy, march })
}

fn g2d(gy: i32, gm: i32, gd: i32) -> i32 {
    let mut day =
        div((gy + div(gm - 8, 6) + 100_100) * 1461, 4) + div(153 * modulo(gm + 9, 12) + 2, 5) + gd
            - 34_840_408;
    day = day - div(div(gy + 100_100 + div(gm - 8, 6), 100) * 3, 4) + 752;
    day
}

fn d2g(day_number: i32) -> (i32, i32, i32) {
    let mut j = 4 * day_number + 139_361_631;
    j = j + div(div(4 * day_number + 183_187_720, 146_097) * 3, 4) * 4 - 3908;
    let i = div(modulo(j, 1461), 4) * 5 + 308;
    let day = div(modulo(i, 153), 5) + 1;
    let month = modulo(div(i, 153), 12) + 1;
    let year = div(j, 1461) - 100_100 + div(8 - month, 6);
    (year, month, day)
}

fn j2d(year: i32, month: i32, day: i32) -> Option<i32> {
    let cal = jal_cal(year)?;
    Some(g2d(cal.gy, 3, cal.march) + (month - 1) * 31 - div(month, 7) * (month - 7) + day - 1)
}

fn d2j(day_number: i32) -> Option<Jalali> {
    let (gy, _, _) = d2g(day_number);
    let mut year = gy - 621;
    let cal = jal_cal(year)?;
    let march = g2d(gy, 3, cal.march);
    let mut offset = day_number - march;

    if offset >= 0 {
        if offset <= 185 {
            return Some(Jalali {
                year,
                month: (1 + div(offset, 31)) as u8,
                day: (modulo(offset, 31) + 1) as u8,
            });
        }
        offset -= 186;
    } else {
        year -= 1;
        offset += 179;
        if cal.leap == 1 {
            offset += 1;
        }
    }

    Some(Jalali {
        year,
        month: (7 + div(offset, 30)) as u8,
        day: (modulo(offset, 30) + 1) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gregorian(year: i32, month: u8, day: u8) -> Gregorian {
        Gregorian { year, month, day }
    }

    #[test]
    fn converts_known_dates() {
        let pairs = [
            (
                Jalali {
                    year: 1348,
                    month: 10,
                    day: 11,
                },
                (1970, 1, 1),
            ),
            (
                Jalali {
                    year: 1402,
                    month: 5,
                    day: 12,
                },
                (2023, 8, 3),
            ),
            (
                Jalali {
                    year: 1403,
                    month: 1,
                    day: 1,
                },
                (2024, 3, 20),
            ),
            (
                Jalali {
                    year: 1399,
                    month: 12,
                    day: 30,
                },
                (2021, 3, 20),
            ),
            (
                Jalali {
                    year: 1402,
                    month: 12,
                    day: 29,
                },
                (2024, 3, 19),
            ),
        ];
        for (jalali, (year, month, day)) in pairs {
            let expected = gregorian(year, month, day);
            assert_eq!(jalali.to_gregorian(), Some(expected), "{jalali:?}");
            assert_eq!(Jalali::from_gregorian(expected), Some(jalali));
        }
    }

    #[test]
    fn round_trips_every_month() {
        for year in 1390..=1420 {
            for month in 1..=12 {
                let day = Jalali::month_days(year, month).unwrap();
                let jalali = Jalali::new(year, month, day).unwrap();
                let gregorian = jalali.to_gregorian().unwrap();
                assert_eq!(Jalali::from_gregorian(gregorian), Some(jalali));
            }
        }
    }

    #[test]
    fn leap_years_follow_the_calendar() {
        assert!(Jalali::is_leap(1399));
        assert!(Jalali::is_leap(1403));
        assert!(!Jalali::is_leap(1402));
        assert_eq!(Jalali::month_days(1403, 12), Some(30));
        assert_eq!(Jalali::month_days(1402, 12), Some(29));
        assert_eq!(Jalali::month_days(1402, 13), None);
        assert_eq!(Jalali::new(1402, 12, 30), None);
    }

    #[test]
    fn parses_numeric_and_named_dates() {
        let expected = Some(Jalali {
            year: 1402,
            month: 5,
            day: 12,
        });
        assert_eq!(parse("۱۴۰۲/۰۵/۱۲"), expected);
        assert_eq!(parse("1402-5-12"), expected);
        assert_eq!(parse("12.5.1402"), expected);
        assert_eq!(parse("۱۲ مرداد ۱۴۰۲"), expected);
        assert_eq!(parse("۱۲ مردادماه ۱۴۰۲."), expected);
        assert_eq!(parse("۱۲ امرداد ۱۴۰۲"), expected);
        assert_eq!(parse("1402/13/01"), None);
        assert_eq!(parse("not a date"), None);
    }

    #[test]
    fn formats_dates() {
        let date = Jalali {
            year: 1402,
            month: 5,
            day: 12,
        };
        assert_eq!(date.format_persian(), "۱۲ مرداد ۱۴۰۲");
        assert_eq!(date.format_numeric(), "۱۴۰۲/۰۵/۱۲");
        assert_eq!(date.to_iso().as_deref(), Some("2023-08-03"));
        assert_eq!(
            format(gregorian(2023, 8, 3)).as_deref(),
            Some("۱۲ مرداد ۱۴۰۲")
        );
    }
}
