use std::collections::VecDeque;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Samples {
    samples: VecDeque<Sample>,
    max_len: usize,
}

impl Samples {
    pub fn with_max_len(max_len: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_len),
            max_len,
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn full(&self) -> bool {
        self.samples.len() == self.max_len
    }

    // https://en.wikipedia.org/wiki/Simple_linear_regression
    pub fn slope(&self) -> Option<f64> {
        // Calculate x and y mean.
        let len = self.samples.len() as f64;
        if len < 2. {
            return None;
        }
        let mut x_mean = 0.;
        let mut y_mean = 0.;
        for entry in &self.samples {
            x_mean += entry.x;
            y_mean += entry.y;
        }
        x_mean /= len;
        y_mean /= len;

        // Calculate slope.
        let mut num = 0.;
        let mut denum = 0.;
        for entry in &self.samples {
            let delta_x = entry.x - x_mean;
            let delta_y = entry.y - y_mean;
            num += delta_x * delta_y;
            denum += delta_x * delta_x;
        }
        if denum != 0. {
            Some(num / denum)
        } else {
            None
        }
    }

    pub fn push(&mut self, sample: Sample) {
        if self.samples.len() == self.max_len {
            self.samples.pop_back();
        }
        self.samples.push_front(sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slope() {
        let test_cases: Vec<(Vec<Sample>, Option<f64>)> = vec![
            (vec![], None),
            (vec![Sample { x: 0., y: 0. }], None),
            (vec![Sample { x: 0., y: 0. }, Sample { x: 0., y: 0. }], None),
            (
                vec![Sample { x: 0., y: 0. }, Sample { x: 1., y: 0. }],
                Some(0.),
            ),
            (
                vec![Sample { x: 0., y: 0. }, Sample { x: 1., y: 1. }],
                Some(1.),
            ),
            (
                vec![Sample { x: 0., y: 0. }, Sample { x: 1., y: -1. }],
                Some(-1.),
            ),
            (
                vec![
                    Sample { x: 0., y: 0. },
                    Sample { x: 1., y: 2. },
                    Sample { x: 2., y: 4. },
                ],
                Some(2.),
            ),
        ];

        for test_case in &test_cases {
            let input = &test_case.0;
            let expected_slope = test_case.1;

            let mut samples = Samples::with_max_len(100);
            for sample in input {
                samples.push(*sample);
            }

            let actual_slope = samples.slope();

            let msg_if_fail =
                format!("input={input:?} actual={actual_slope:?} expected={expected_slope:?}");

            if let Some(slope) = actual_slope {
                const EPSILON: f64 = 0.000001;
                let expected = expected_slope.unwrap_or_else(|| panic!("{msg_if_fail}"));
                assert!((slope - expected).abs() < EPSILON, "{}", msg_if_fail);
            } else {
                assert_eq!(expected_slope, None)
            }
        }
    }
}
