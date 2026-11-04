//! Original prototype chain scoring, independent of Skate 3's scoring rules.
pub const BANK_DELAY: f32 = 1.5;

#[derive(Debug, Default, Clone)]
pub struct Combo {
    pub base_points: u32,
    pub tricks: u32,
    pub remaining: f32,
}
impl Combo {
    pub fn multiplier(&self) -> u32 {
        self.tricks.min(8)
    }
    pub fn value(&self) -> u32 {
        self.base_points.saturating_mul(self.multiplier())
    }
    pub fn add(&mut self, points: u32) {
        if points == 0 {
            return;
        }
        self.base_points = self.base_points.saturating_add(points);
        self.tricks = self.tricks.saturating_add(1);
        self.remaining = BANK_DELAY;
    }
    /// Only called for time spent safely riding. Air and grinds pause banking.
    pub fn ride(&mut self, dt: f32) -> u32 {
        if !dt.is_finite() || dt <= 0.0 || self.tricks == 0 {
            return 0;
        }
        self.remaining = (self.remaining - dt).max(0.0);
        if self.remaining > 0.00001 {
            return 0;
        }
        let points = self.value();
        *self = Self::default();
        points
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linked_tricks_bank_once_and_saturation_is_bounded() {
        let mut combo = Combo::default();
        combo.add(400);
        assert_eq!(combo.ride(0.5), 0);
        combo.add(600);
        assert_eq!(combo.value(), 2000);
        assert_eq!(combo.ride(1.0), 0);
        assert_eq!(combo.ride(0.5), 2000);
        assert_eq!(combo.ride(2.0), 0);
        for _ in 0..12 {
            combo.add(u32::MAX);
        }
        assert_eq!(combo.multiplier(), 8);
        assert_eq!(combo.value(), u32::MAX);
        combo.clear();
        assert_eq!(combo.value(), 0);
    }
    #[test]
    fn banks_after_exactly_180_fixed_steps_and_ignores_invalid_time() {
        let mut combo = Combo::default();
        combo.add(100);
        for dt in [0.0, -1.0, f32::NAN] {
            assert_eq!(combo.ride(dt), 0);
        }
        for _ in 0..179 {
            assert_eq!(combo.ride(crate::skating::FIXED_DT), 0);
        }
        assert_eq!(combo.ride(crate::skating::FIXED_DT), 100);
    }
}
