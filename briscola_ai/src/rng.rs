#[derive(Debug, Clone)]
pub struct FastRng {
    state: u64,
}

impl FastRng {
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
        Self { state }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn gen_below(&mut self, upper_exclusive: usize) -> usize {
        if upper_exclusive <= 1 {
            return 0;
        }

        let Ok(upper_u64) = u64::try_from(upper_exclusive) else {
            return 0;
        };
        let value_u64 = self.next_u64() % upper_u64;

        usize::try_from(value_u64).unwrap_or_default()
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        if items.len() < 2 {
            return;
        }

        let mut i = items.len() - 1;
        while i > 0 {
            let j = self.gen_below(i + 1);
            items.swap(i, j);
            i -= 1;
        }
    }
}
