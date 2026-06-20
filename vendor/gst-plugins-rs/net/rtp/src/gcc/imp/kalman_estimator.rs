//! This is the estimator that follows the algorithm described in
//! https://datatracker.ietf.org/doc/html/draft-ietf-rmcat-gcc-02.

use super::Duration;
use super::EstimatorImpl;
use super::PacketGroup;

// Table1. Coefficient used for the measured noise variance
//  [0.1,0.001]
const CHI: f64 = 0.01;
const ONE_MINUS_CHI: f64 = 1. - CHI;

// Table1. State noise covariance matrix
const Q: f64 = 0.001;

// Table1. Initial value of the system error covariance
const INITIAL_ERROR_COVARIANCE: f64 = 0.1;

#[derive(Debug, PartialEq, Clone)]
pub struct KalmanEstimator {
    measure: Duration, // Delay variation measure
    gain: f64,
    measurement_uncertainty: f64, // var_v_hat(i-1)
    estimate_error: f64,          // e(i-1)
    estimate: Duration,           // m_hat(i-1)
}

impl Default for KalmanEstimator {
    fn default() -> Self {
        Self {
            measure: Duration::ZERO,
            gain: 0.,
            measurement_uncertainty: 0.,
            estimate_error: INITIAL_ERROR_COVARIANCE,
            estimate: Duration::ZERO,
        }
    }
}

impl EstimatorImpl for KalmanEstimator {
    fn update(&mut self, prev_group: &PacketGroup, group: &PacketGroup) {
        self.measure = group.inter_delay_variation(prev_group);

        let z = self.measure - self.estimate;
        let zms = z.whole_microseconds() as f64 / 1000.0;

        // This doesn't exactly follows the spec as we should compute and
        // use f_max here, no implementation we have found actually uses it.
        let alpha = ONE_MINUS_CHI.powf(30.0 / (1000. * 5. * 1_000_000.));
        let root = self.measurement_uncertainty.sqrt();
        let root3 = 3. * root;

        if zms > root3 {
            self.measurement_uncertainty =
                (alpha * self.measurement_uncertainty + (1. - alpha) * root3.powf(2.)).max(1.);
        } else {
            self.measurement_uncertainty =
                (alpha * self.measurement_uncertainty + (1. - alpha) * zms.powf(2.)).max(1.);
        }

        let estimate_uncertainty = self.estimate_error + Q;
        self.gain = estimate_uncertainty / (estimate_uncertainty + self.measurement_uncertainty);
        self.estimate += Duration::nanoseconds((self.gain * zms * 1_000_000.) as i64);
        self.estimate_error = (1. - self.gain) * estimate_uncertainty;
    }

    fn estimate(&self) -> Duration {
        self.estimate
    }

    fn measure(&self) -> Duration {
        self.measure
    }
}
