// The shapes of java.time's hierarchies: a value read through a wildcard has its type
// variable's bound (`other.date()` on a `Stamp<?>` is a `Day`, as `ChronoLocalDateTime.isAfter`
// reads `toLocalDate()`); an interface's method whose implementation reassigns its parameter
// with a value that may be null (`TemporalAmount.addTo` and `Period.addTo`); and an enum
// constant's own method that passes a class's object for an interface parameter
// (`IsoFields.Field.resolve` and `rangeRefinedBy`).
public class DateLikeHierarchies {
    interface Moment {
        Moment plus(long days);

        long days();
    }

    interface Day extends Moment {
        default long epochDay() {
            return days();
        }
    }

    static final class Date implements Day {
        final long days;

        Date(long days) {
            this.days = days;
        }

        public Date plus(long more) {
            return more == 0 ? this : new Date(days + more);
        }

        public long days() {
            return days;
        }

        public String toString() {
            return "day " + days;
        }
    }

    static final class JulianDay implements Day {
        final long number;

        JulianDay(long number) {
            this.number = number;
        }

        public JulianDay plus(long more) {
            return new JulianDay(number + more);
        }

        public long days() {
            return number - 100;
        }

        public String toString() {
            return "julian " + number;
        }
    }

    static final class Instant implements Moment {
        final long seconds;

        Instant(long seconds) {
            this.seconds = seconds;
        }

        public Instant plus(long days) {
            return days < 0 ? null : new Instant(seconds + days * 86400);
        }

        public long days() {
            return seconds / 86400;
        }

        public String toString() {
            return "instant " + seconds;
        }
    }

    interface Stamp<D extends Day> {
        D date();

        long seconds();

        default boolean isAfter(Stamp<?> other) {
            long mine = date().epochDay();
            long theirs = other.date().epochDay();
            return mine > theirs || mine == theirs && seconds() > other.seconds();
        }
    }

    record DateStamp(Date date, long seconds) implements Stamp<Date> {}

    record JulianStamp(JulianDay date, long seconds) implements Stamp<JulianDay> {}

    interface Amount {
        Moment addTo(Moment moment);
    }

    static final class Weeks implements Amount {
        final int weeks;

        Weeks(int weeks) {
            this.weeks = weeks;
        }

        public Moment addTo(Moment moment) {
            if (weeks != 0) {
                moment = moment.plus(7L * weeks);
            }
            return moment;
        }
    }

    static final class Days implements Amount {
        public Moment addTo(Moment moment) {
            return moment.plus(1);
        }
    }

    enum Field {
        DAY_OF_WEEK {
            long range(Moment moment) {
                return moment.days() % 7;
            }

            long check(Date date) {
                return range(date) + 1;
            }
        },
        WEEK {
            long range(Moment moment) {
                return moment.days() / 7;
            }

            long check(Date date) {
                return range(date) * 2 + range(new Instant(864000));
            }
        };

        abstract long range(Moment moment);

        abstract long check(Date date);
    }

    public static void main(String[] args) {
        Stamp<?> early = new DateStamp(new Date(10), 5);
        Stamp<?> julian = new JulianStamp(new JulianDay(110), 7);
        Stamp<Date> late = new DateStamp(new Date(10), 9);
        System.out.println(late.isAfter(early) + " " + early.isAfter(late) + " " + julian.isAfter(early));
        Amount[] amounts = {new Weeks(2), new Weeks(0), new Weeks(-1), new Days()};
        Moment[] moments = {new Date(3), new JulianDay(150), new Instant(86400)};
        for (Amount amount : amounts) {
            for (Moment moment : moments) {
                System.out.println(amount.addTo(moment));
            }
        }
        for (Field field : Field.values()) {
            System.out.println(field + " " + field.check(new Date(20)));
        }
    }
}
