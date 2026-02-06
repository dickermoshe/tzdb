use core::fmt::{Debug, Display, Error, Formatter};

use chrono::{
    Datelike, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone,
    Timelike,
};
use tz::{DateTime as TzDateTime, LocalTimeType};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// A struct which wraps a time zone from the tz-rs crate.
///
/// This is used to implement the TimeZone trait from chrono.
pub struct WrappedTz {
    /// The wrapped time zone.
    pub tz: tz::TimeZoneRef<'static>,
}

impl TimeZone for WrappedTz {
    type Offset = TzOffset;

    fn from_offset(offset: &Self::Offset) -> Self {
        offset.tz
    }

    #[allow(deprecated)]
    fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<Self::Offset> {
        let earliest = self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN));
        let latest = self.offset_from_local_datetime(&local.and_hms_opt(23, 59, 59).unwrap());
        // From the chrono docs:
        //
        // > This type should be considered ambiguous at best, due to the inherent lack of
        // > precision required for the time zone resolution. There are some guarantees on the usage
        // > of `Date<Tz>`:
        // > - If properly constructed via `TimeZone::ymd` and others without an error,
        // >   the corresponding local date should exist for at least a moment.
        // >   (It may still have a gap from the offset changes.)
        //
        // > - The `TimeZone` is free to assign *any* `Offset` to the local date,
        // >   as long as that offset did occur in given day.
        // >   For example, if `2015-03-08T01:59-08:00` is followed by `2015-03-08T03:00-07:00`,
        // >   it may produce either `2015-03-08-08:00` or `2015-03-08-07:00`
        // >   but *not* `2015-03-08+00:00` and others.
        //
        // > - Once constructed as a full `DateTime`,
        // >   `DateTime::date` and other associated methods should return those for the original `Date`.
        // >   For example, if `dt = tz.ymd(y,m,d).hms(h,n,s)` were valid, `dt.date() == tz.ymd(y,m,d)`.
        //
        // > - The date is timezone-agnostic up to one day (i.e. practically always),
        // >   so the local date and UTC date should be equal for most cases
        // >   even though the raw calculation between `NaiveDate` and `Duration` may not.
        //
        // For these reasons we return always a single offset here if we can, rather than being
        // technically correct and returning Ambiguous(_,_) on days when the clock changes. The
        // alternative is painful errors when computing unambiguous times such as
        // `TimeZone.ymd(ambiguous_date).hms(unambiguous_time)`.
        use chrono::LocalResult::*;
        match (earliest, latest) {
            (result @ Single(_), _) => result,
            (_, result @ Single(_)) => result,
            (Ambiguous(offset, _), _) => Single(offset),
            (_, Ambiguous(offset, _)) => Single(offset),
            (None, None) => None,
        }
    }

    // First search for a timespan that the local datetime falls into, then, if it exists,
    // check the two surrounding timespans (if they exist) to see if there is any ambiguity.
    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<Self::Offset> {
        let year = local.year();
        let month = local.month();
        let month_day = local.day();
        let hour = local.hour();
        let minute = local.minute();
        let second = local.second();
        let nanoseconds = local.nanosecond();
        let tzdt = TzDateTime::find(
            year,
            month as u8,
            month_day as u8,
            hour as u8,
            minute as u8,
            second as u8,
            nanoseconds,
            self.tz,
        );
        if let Ok(tzdt) = tzdt {
            if let Some(tzdt) = tzdt.unique() {
                return LocalResult::Single(TzOffset::new(*self, *tzdt.local_time_type()));
            } else if let (Some(earliest), Some(latest)) = (tzdt.earliest(), tzdt.latest()) {
                return LocalResult::Ambiguous(
                    TzOffset::new(*self, *earliest.local_time_type()),
                    TzOffset::new(*self, *latest.local_time_type()),
                );
            } else {
                return LocalResult::None;
            }
        }
        return LocalResult::None;
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> Self::Offset {
        let naive_dt = utc.and_hms_opt(0, 0, 0).unwrap();
        return self.offset_from_utc_datetime(&naive_dt);
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
        let dt = TzDateTime::from_timespec(utc.and_utc().timestamp(), utc.nanosecond(), self.tz)
            .unwrap();
        TzOffset::new(*self, *dt.local_time_type())
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct TzOffset {
    tz: WrappedTz,
    offset: LocalTimeType,
}

impl TzOffset {
    fn new(tz: WrappedTz, offset: LocalTimeType) -> Self {
        TzOffset { tz, offset }
    }
}

impl Offset for TzOffset {
    fn fix(&self) -> FixedOffset {
        FixedOffset::east_opt(self.offset.ut_offset()).unwrap()
    }
}

impl Display for TzOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Display::fmt(&self.offset.time_zone_designation(), f)
    }
}

impl Debug for TzOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Debug::fmt(&self.offset.time_zone_designation(), f)
    }
}

impl defmt::Format for TzOffset {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "{:?}", self.offset.time_zone_designation());
    }
}
