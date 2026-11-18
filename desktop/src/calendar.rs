//! Dates for the journal: the month grid the calendar shows, and the `YYYY-MM-DD` names of
//! day notes.

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// A calendar day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Day {
    pub fn new(year: i32, month: u32, day: u32) -> Day {
        Day { year, month, day }
    }

    /// The local date now.
    pub fn today() -> Day {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let t = rune::LocalTime::at(ms);
        Day::new(t.year, t.month, t.day)
    }

    /// The day as a note name: `2026-11-17`.
    pub fn name(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// The day as a journal entry's title: `Tuesday 17 November`.
    pub fn long(self) -> String {
        format!("{} {} {}", DAYS[weekday(self.year, self.month, self.day) as usize], self.day, MONTHS[(self.month - 1) as usize])
    }
}

/// The day a note name such as `2026-11-17` stands for.
pub fn parse(name: &str) -> Option<Day> {
    let b = name.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| name.get(r).filter(|s| s.bytes().all(|c| c.is_ascii_digit()))?.parse::<u32>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    ((1..=12).contains(&m) && (1..=days_in_month(y as i32, m)).contains(&d)).then(|| Day::new(y as i32, m, d))
}

pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

/// The weekday of a date, 0 for Sunday, by Sakamoto's method.
pub fn weekday(year: i32, month: u32, day: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    (y + y / 4 - y / 100 + y / 400 + T[(month - 1) as usize] + day as i32).rem_euclid(7) as u32
}

/// The month before or after `(year, month)`, `by` months away.
pub fn add_months(year: i32, month: u32, by: i32) -> (i32, u32) {
    let i = year * 12 + month as i32 - 1 + by;
    (i.div_euclid(12), (i.rem_euclid(12) + 1) as u32)
}

/// The month as weeks of seven cells, Sunday first. Cells outside the month are `None`.
pub fn grid(year: i32, month: u32) -> Vec<[Option<u32>; 7]> {
    let first = weekday(year, month, 1) as usize;
    let n = days_in_month(year, month) as usize;
    let mut weeks = Vec::new();
    let mut week = [None; 7];
    for i in 0..first + n {
        if i >= first {
            week[i % 7] = Some((i - first + 1) as u32);
        }
        if i % 7 == 6 {
            weeks.push(week);
            week = [None; 7];
        }
    }
    if week.iter().any(Option::is_some) {
        weeks.push(week);
    }
    weeks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_and_titles() {
        assert_eq!(weekday(2026, 11, 17), 2);
        assert_eq!(weekday(2000, 1, 1), 6);
        assert_eq!(Day::new(2026, 11, 17).long(), "Tuesday 17 November");
        assert_eq!(Day::new(2026, 3, 5).name(), "2026-03-05");
    }

    #[test]
    fn month_lengths_and_steps() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(add_months(2026, 12, 1), (2027, 1));
        assert_eq!(add_months(2026, 1, -1), (2025, 12));
        assert_eq!(add_months(2026, 11, -23), (2024, 12));
    }

    #[test]
    fn the_grid_starts_on_sunday() {
        // November 2026 starts on a Sunday and takes five weeks.
        let nov = grid(2026, 11);
        assert_eq!(nov.len(), 5);
        assert_eq!(nov[0][0], Some(1));
        assert_eq!(nov[4], [Some(29), Some(30), None, None, None, None, None]);
        // August 2026 starts on a Saturday and takes six.
        let aug = grid(2026, 8);
        assert_eq!(aug.len(), 6);
        assert_eq!(aug[0], [None, None, None, None, None, None, Some(1)]);
        assert_eq!(aug[5][1], Some(31));
        // February 2026 fills exactly four weeks; 2024's has a 29th.
        assert_eq!(grid(2026, 2).len(), 4);
        assert!(grid(2024, 2).iter().flatten().any(|d| *d == Some(29)));
    }

    #[test]
    fn day_names_parse() {
        assert_eq!(parse("2026-11-17"), Some(Day::new(2026, 11, 17)));
        assert_eq!(parse("2024-02-29"), Some(Day::new(2024, 2, 29)));
        assert_eq!(parse("2026-02-29"), None);
        assert_eq!(parse("2026-13-01"), None);
        assert_eq!(parse("2026-1-01"), None);
        assert_eq!(parse("Meeting 2026-11-17"), None);
        assert_eq!(parse("2026-11-1a"), None);
    }
}
