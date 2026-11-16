//! Seat is a small index into the players around the table.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seat(u8);

impl Seat {
    pub fn new(index: u8) -> Seat {
        Seat(index)
    }

    pub fn index(self) -> u8 {
        self.0
    }

    /// The next seat in rotation around a table of `count` seats.
    pub fn next(self, count: u8) -> Seat {
        debug_assert!(count > 0, "seat count must be positive");
        Seat((self.0 + 1) % count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_wraps_two_seats() {
        assert_eq!(Seat::new(0).next(2), Seat::new(1));
        assert_eq!(Seat::new(1).next(2), Seat::new(0));
    }

    #[test]
    fn next_wraps_three_seats() {
        assert_eq!(Seat::new(0).next(3), Seat::new(1));
        assert_eq!(Seat::new(1).next(3), Seat::new(2));
        assert_eq!(Seat::new(2).next(3), Seat::new(0));
    }

    #[test]
    fn next_wraps_four_seats() {
        for start in 0..4 {
            let after = Seat::new(start).next(4);
            assert_eq!(after.index(), (start + 1) % 4);
        }
    }

    #[test]
    fn full_rotation_returns_to_start() {
        let start = Seat::new(1);
        let mut s = start;
        for _ in 0..4 {
            s = s.next(4);
        }
        assert_eq!(s, start);
    }
}
